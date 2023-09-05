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

use crate::Directive;

pub fn parse(
    context: &mut rhai::EvalContext<'_, '_, '_, '_, '_, '_>,
    input: &[rhai::Expression<'_>],
    _state: &rhai::Dynamic,
) -> crate::api::Result<rhai::Dynamic> {
    let name = input[0]
        .get_literal_value::<rhai::ImmutableString>()
        .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
            "rule name must be a string".to_string().into()
        })?;
    let expr = context.eval_expression_tree(&input[1])?;

    Ok(rhai::Dynamic::from(Directive::Rule {
        name: name.to_string(),
        pointer: {
            match expr.try_cast::<rhai::FnPtr>() {
                Some(ptr) => ptr,
                None => {
                    return Err("a rule must end with a closure or a function pointer"
                        .to_string()
                        .into());
                }
            }
        },
    }))
}
