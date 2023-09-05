/*
 * vSMTP mail transfer agent
 *
 * Copyright (C) 2003 - viridIT SAS
 * Licensed under the Elastic License 2.0 
 *
 * You should have received a copy of the Elastic License 2.0 along with 
 * this program. If not, see https://www.elastic.co/licensing/elastic-license.
 *
 */

use crate::{directives_try_from, rhai, Directives, Stage};
use vsmtp_protocol::Domain;

// PERF: Directives are cloned every time they are returned to make the
//       code easy to reason about. We could wrap them in an `Arc`,
//       even cache rules by comparing the arc pointers to make it
//       less memory intensive.

/// Domains and their associated stages and rules.
pub type Domains<STAGE> = std::collections::BTreeMap<Domain, DomainStages<STAGE>>;

/// Set of rules for each stages of a single domain.
pub struct DomainStages<STAGE: Stage>(pub std::collections::BTreeMap<STAGE, Directives>);

impl<STAGE: Stage> TryFrom<rhai::Dynamic> for DomainStages<STAGE> {
    type Error = Box<rhai::EvalAltResult>;

    fn try_from(value: rhai::Dynamic) -> Result<Self, Self::Error> {
        let t = value.type_name();
        value
            .try_cast::<rhai::Map>()
            .ok_or_else(|| {
                Box::new(rhai::EvalAltResult::ErrorParsing(
                    rhai::ParseErrorType::MismatchedType("map of stages".to_string(), t.to_owned()),
                    rhai::Position::NONE,
                ))
            })?
            .into_iter()
            .map(|(k, v)| {
                STAGE::from_str(k.as_str())
                    .map_err(|_| format!("unknown stage '{k}'").into())
                    .and_then(|stage| {
                        directives_try_from(v, Some(&stage))
                            .map(|set| (stage, set))
                            .map_err(|err| err.kind.to_string().into())
                    })
            })
            .collect::<Result<std::collections::BTreeMap<STAGE, Directives>, Self::Error>>()
            .map(Self)
    }
}

/// Resolver used to parse `vSMTP` rules scripts and split those by domain.
///
/// Resolve any module in the given directory that contains a `rules` variable.
/// All resolved scripts must either be:
///
/// - a script with a domain for it's name. (e.g `example.com.rhai`)
/// - a directory with a domain for it's name and scripts with stages for their name. (e.g `example.com/rcpt_to.rhai`)
pub struct DomainFilterResolver<STAGE: Stage> {
    root: std::path::PathBuf,
    stages: std::marker::PhantomData<STAGE>,
}

impl<STAGE: Stage> DomainFilterResolver<STAGE> {
    /// Build a new resolver.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            root: path.into(),
            stages: std::marker::PhantomData,
        }
    }

    /// Extract a rule set from a module.
    fn get_rule_set(module: &rhai::Module) -> Result<Directives, Box<rhai::EvalAltResult>> {
        directives_try_from(
            module
                .get_var("rules")
                .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                    "missing exported object 'rules' in module".into()
                })?,
            None::<&STAGE>,
        )
        .map_err(Into::into)
    }

    /// Extract all rules for a single domain from a module.
    fn get_domain_rules(
        module: &rhai::Module,
    ) -> Result<DomainStages<STAGE>, Box<rhai::EvalAltResult>> {
        DomainStages::try_from(
            module
                .get_var("rules")
                .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                    "missing exported object 'rules' in module".into()
                })?,
        )
    }

    /// Return a module specific error, used for the `resolve` method.
    ///
    /// NOTE: every error are wrapped in `module_error` because a specific variant
    ///       of a rhai error needs to be returned when `resolve` is called.
    #[allow(clippy::unnecessary_box_returns)]
    fn module_error(
        &self,
        error: Box<rhai::EvalAltResult>,
        pos: rhai::Position,
    ) -> Box<rhai::EvalAltResult> {
        Box::new(rhai::EvalAltResult::ErrorInModule(
            format!("{:?}", self.root),
            error,
            pos,
        ))
    }
}

impl<STAGE: Stage + 'static> rhai::ModuleResolver for DomainFilterResolver<STAGE> {
    /// Resolve and import a module.
    /// <https://rhai.rs/book/ref/modules/import.html#import-statement>
    fn resolve(
        &self,
        engine: &rhai::Engine,
        _source: Option<&str>,
        path: &str,
        pos: rhai::Position,
    ) -> Result<rhai::Shared<rhai::Module>, Box<rhai::EvalAltResult>> {
        let mut rules: Domains<STAGE> = Domains::default();
        let mut module = rhai::Module::new();

        let root = self.root.join(path);

        // If the file is not a directory, this is probably a regular script,
        // thus we can skip it.
        if !root.is_dir() {
            return Err(Box::new(rhai::EvalAltResult::ErrorModuleNotFound(
                format!("{root:?}"),
                pos,
            )));
        }

        let stages = STAGE::stages();

        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .skip(1)
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let file_path = entry.path();
            let stem = file_path
                .file_stem()
                .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                    self.module_error(
                        format!("the file name of {file_path:?} is invalid").into(),
                        pos,
                    )
                })?
                .to_string_lossy();
            let mut global = rhai::GlobalRuntimeState::new(engine);
            let mut scope = rhai::Scope::new();

            let ast = engine
                .compile_file_with_scope(&scope, file_path.to_path_buf())
                .map_err(|error| self.module_error(error, pos))?;

            let rules_module =
                rhai::Module::eval_ast_as_new_raw(engine, &mut scope, &mut global, &ast)
                    .map_err(|err| self.module_error(err, pos))?;

            if stages.contains(&stem.as_ref()) {
                // The parent directory must be named after the domain.
                let domain = file_path
                    .parent()
                    .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                        self.module_error(
                            "parent directory for domain script does not exists".into(),
                            pos,
                        )
                    })?
                    .file_name()
                    .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                        self.module_error(
                            "final component of script parent directory does not exists".into(),
                            pos,
                        )
                    })?
                    .to_string_lossy()
                    .parse::<Domain>()
                    .map_err::<Box<rhai::EvalAltResult>, _>(|error| {
                        self.module_error(
                            format!("parent directory of a script must be a domain name: {error}")
                                .into(),
                            pos,
                        )
                    })?;

                // Rules are fetched from the compiled file.
                // Since the name of the file is already a stage,
                // the exported rule variable is an array of rules
                // or a map of the email flow.
                let rule_set =
                    Self::get_rule_set(&rules_module).map_err(|err| self.module_error(err, pos))?;

                let Ok(stage) = STAGE::from_str(stem.as_ref()) else {
                    return Err(self.module_error(
                        format!("the '{stem}' file name is not a stage").into(),
                        pos,
                    ));
                };

                if let Some(domain_rules) = rules.get_mut(&domain) {
                    domain_rules.0.insert(stage, rule_set);
                } else {
                    rules.insert(
                        domain,
                        DomainStages(std::iter::once((stage, rule_set)).collect()),
                    );
                }
            } else if let Ok(domain) = stem.parse::<Domain>() {
                // When the file is a domain, we parse the rule variable with it's stage directly.
                rules.insert(
                    domain,
                    // Since there is a single file for this domain,
                    // rules must be split by stages.
                    Self::get_domain_rules(&rules_module)
                        .map_err(|err| self.module_error(err, pos))?,
                );
            }

            module.combine(rules_module);
        }

        module
            .set_var("rules", rhai::Shared::new(rules))
            .build_index();

        Ok(rhai::Shared::new(module))
    }
}
