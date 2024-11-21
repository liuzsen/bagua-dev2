use std::sync::Mutex;

use bagua::InitFunction;
use settings::Settings;

mod settings {
    pub struct Settings {}
}

mod init {
    use linkme::distributed_slice;

    use crate::settings::Settings;

    type BoxedInitFn = for<'a> fn(
        &'a Settings,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<()>> + 'a>,
    >;

    #[distributed_slice]
    pub static MACROS_INIT_FUNCTIONS: [InitFunction];

    pub async fn run_all(s: &Settings) -> anyhow::Result<()> {
        let mut methods = MACROS_INIT_FUNCTIONS.iter().collect::<Vec<_>>();
        methods.sort_by_key(|f| f.priority);

        for init_fn in methods {
            init_fn.call(s).await?;
        }
        Ok(())
    }

    pub struct InitFunction {
        pub function: BoxedInitFn,
        pub priority: u8,
    }

    impl InitFunction {
        async fn call(&self, s: &Settings) -> anyhow::Result<()> {
            (self.function)(s).await
        }
    }

    pub struct InitFunctionBuilder {
        method: BoxedInitFn,
        priority: Option<u8>,
    }

    impl InitFunctionBuilder {
        pub const fn new(method: BoxedInitFn) -> Self {
            Self {
                method,
                priority: None,
            }
        }

        pub const fn priority(mut self, priority: u8) -> Self {
            self.priority = Some(priority);
            self
        }

        pub const fn build(self) -> InitFunction {
            let priority = match self.priority {
                Some(priority) => priority,
                None => u8::MAX / 2,
            };

            InitFunction {
                function: self.method,
                priority,
            }
        }
    }
}

static INIT_FUNCTIONS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

#[InitFunction(priority = 1)]
async fn aa(_s: &Settings) -> anyhow::Result<()> {
    INIT_FUNCTIONS.lock().unwrap().push("aa");

    Ok(())
}

#[InitFunction(priority = 3)]
async fn bb(_s: &Settings) -> anyhow::Result<()> {
    INIT_FUNCTIONS.lock().unwrap().push("bb");

    Ok(())
}

#[InitFunction(priority = 2)]
async fn cc(_s: &Settings) -> anyhow::Result<()> {
    INIT_FUNCTIONS.lock().unwrap().push("cc");

    Ok(())
}

#[tokio::test]
async fn test_init_functions() -> anyhow::Result<()> {
    let s = Settings {};
    init::run_all(&s).await?;

    let functions = INIT_FUNCTIONS.lock().unwrap();
    assert_eq!(&*functions, &*vec!["aa", "cc", "bb"]);

    Ok(())
}
