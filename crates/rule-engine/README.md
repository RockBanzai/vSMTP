# vSMTP rule engine

vSMTP server is built with filtering rules also called `rule engine`.
This runtime can execute code provided in our superset of the
<https://rhai.rs> language.

Rhai is a simple scripting language that allows one to define
`rules` and `object` to control the traffic of vSMTP.

Further details on the official book of vSMTP: <https://vsmtp.rs/reference/rhai/rhai.html>

## How to use

```rust
use rhai::plugin::*;
use rule_engine::*;


#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MyStages {
    Helo,
    Ehlo,
}

impl Stage for MyStages {
    fn hook(&self) -> &'static str {
        match self {
            Self::Helo => "on_helo",
            Self::Ehlo => "on_ehlo",
        }
    }

    fn stages() -> &'static [&'static str] {
        &["helo", "ehlo"]
    }
}

impl std::str::FromStr for MyStages {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "helo" => Ok(Self::Helo),
            "ehlo" => Ok(Self::Ehlo),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for MyStages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                MyStages::Helo => "helo",
                MyStages::Ehlo => "ehlo",
            }
        )
    }
}


// Defined a rhai module use to print to stdout.
#[rhai::export_module]
mod module1 {
    pub fn hello() {
        println!("hello world!");
    }

    pub fn print(message: &str) {
        println!("{message}");
    }
}

// Defined a rhai module use to perform maths.
#[rhai::export_module]
mod module2 {
    pub fn add(x: rhai::INT, y: rhai::INT) -> rhai::INT {
        x + y
    }
}

// Define a rhai module used to mutate an object.
#[rhai::export_module]
mod module3 {
    // A struct that will be mutated using the following functions.
    #[derive(Clone)]
    pub struct MyData(pub rhai::INT);

    pub type Data = MyData;

    pub fn new_data() -> Data {
        MyData(0)
    }

    #[rhai_fn(global, pure)]
    pub fn inc(data: &mut Data) {
        data.0 += 1;
    }

    #[rhai_fn(get = "value", pure)]
    pub fn value(data: &mut Data) -> rhai::INT {
        data.0
    }
}

/// Custom status that rules must return.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
pub enum MyStatus {
    Next,
    Accept,
    FAccept,
    Deny(Option<String>),
    Reject,
}

/// Implement the [`Status`] trait and defining our own rules
/// for each status.
impl Status for MyStatus {
    /// What status is returned when no scripts is provided ?
    fn no_rules(_: &impl Stage) -> Self {
        Self::Deny(None)
    }

    /// What status is returned when an error is raised ?
    fn error(context: rule_engine::DirectiveError) -> Self {
        Self::Deny(Some(context.kind.to_string()))
    }

    /// What status is returned when everything is ok ?
    fn next() -> Self {
        Self::Next
    }
}

// Enable the user to access our statuses via his scripts.
#[rhai::export_module]
mod status {
    pub fn next() -> MyStatus {
        MyStatus::Next
    }
    pub fn accept() -> MyStatus {
        MyStatus::Accept
    }
    pub fn faccept() -> MyStatus {
        MyStatus::FAccept
    }
    pub fn deny() -> MyStatus {
        MyStatus::Deny(None)
    }
    pub fn reject() -> MyStatus {
        MyStatus::Reject
    }
}

/// Define a service configuration.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct MyConfig {
    dummy: bool,
}

/// Everything is unimplemented for the sake of the example.
impl config::Config for MyConfig {
    fn with_path(_path: &impl AsRef<std::path::Path>) -> config::ConfigResult<Self>
    where
        Self: config::Config + serde::de::DeserializeOwned + serde::Serialize,
    {
        Ok(Self::default())
    }

    fn api_version(&self) -> &config::semver::VersionReq {
        unimplemented!()
    }

    fn amqp(&self) -> &config::amqp::AMQP {
        unimplemented!()
    }

    fn queues(&self) -> &config::queues::Queues {
        unimplemented!()
    }

    fn logs(&self) -> &config::logs::Logs {
        unimplemented!()
    }

    fn path(&self) -> &std::path::Path {
        unimplemented!()
    }
}

fn main() {
    let from_manifest_path = |path| std::path::PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), path]);
    let config = MyConfig::default();

    let rule_engine_config = std::sync::Arc::new(
        // We build a rule engine configuration, which serves as a "template" to create the same engine
        // in multi-threaded environment at no cost.
        RuleEngineConfigBuilder::<rhai::Dynamic, MyStatus, MyStages>::default()
            // The configuration is injected as a `config` object.
            .with_configuration(&config)
            .expect("failed to build the configuration")
            // `import` rhai statements will set their root to the `scripts` directory.
            .with_default_module_resolvers(from_manifest_path("tests/scripts"))
            // Import global modules.
            .with_global_modules([rhai::exported_module!(module1).into()])
            // Import static modules.
            .with_static_modules([
                ("maths".to_string(), rhai::exported_module!(module2).into()),
                ("data".to_string(), rhai::exported_module!(module3).into()),
                ("status".to_string(), rhai::exported_module!(status).into()),
            ])
            .with_smtp_modules()
            .with_standard_global_modules()
            // Load the entrypoint script.
            .with_script_at(from_manifest_path("tests/scripts/simple.rhai"))
            .expect("failed to build script simple.rhai")
            .build(),
    );

    // Generate a new engine with the config, inject an empty state.
    // The state is passed as a parameter of each directive closure.
    let rule_engine = RuleEngine::from_config_with_state(
        rule_engine_config,
        std::sync::Arc::new(std::sync::RwLock::new(rhai::Dynamic::UNIT)),
    );

    assert_eq!(rule_engine.run(&MyStages::Helo), MyStatus::Accept);
    assert_eq!(rule_engine.run(&MyStages::Ehlo), MyStatus::Deny(Some("rhai execution produced an error: Function not found: xxx () (line 28, position 13)\nin closure call (line 13, position 9)".to_string())));
}
```
