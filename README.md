# OnceInit

## 例

``` rust
#[cfg(test)]
mod tests {

    // log 门面库的类似实现。
    mod log {
        use crate::{
            OnceInit,
            StaticDefault,
        };
        pub trait Logger: Send + Sync {
            fn log(&self, msg: &str);
        }
        pub static LOGGER: OnceInit<dyn Logger> = OnceInit::new();
        
        // 只有 `T` 实现了 `StaticDefault`, `OnceInit<T>` 才会实现 `Deref<Target = T>`.
        struct DefaultLogger;
        impl Logger for DefaultLogger {
            fn log(&self, _msg: &str) {
                // do nothing.
            }
        }
        impl StaticDefault for dyn Logger {
            fn static_default() -> &'static Self {
                static NOP: DefaultLogger = DefaultLogger;
                &NOP
            }
        }
    }
    mod a_logger {
        use crate::OnceInitError;
        // 一个简单的 a_logger crate.
        use super::log::{
            Logger,
            LOGGER,
        };
        pub struct ALogger;

        impl Logger for ALogger {
            fn log(&self, msg: &str) {
                println!("{msg}");
            }
        }

        impl ALogger {
            pub fn init() -> Result<(), OnceInitError> {
                LOGGER.set_boxed_data(Box::new(ALogger))
            }
        }
    }
    mod hello_world {
        use crate::tests::log::LOGGER;

        pub fn hello_world() {
            LOGGER.log("Hello, world!");
        }
    }
    #[test]
    fn test_logger() {
        a_logger::ALogger::init().unwrap();
        hello_world::hello_world();
    }
}
```

# LICENSE

见[LICENSE](./LICENSE)