// MIT License
//
// Copyright (c) 2025 worksoup <https://github.com/worksoup/>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

// log 门面库的类似实现。
mod log {
    use crate::{OnceInit, StaticDefault};
    pub trait Logger: Send + Sync {
        fn log(&self, msg: &str);
    }
    pub static LOGGER: OnceInit<dyn Logger> = OnceInit::uninit();

    // 只有 `T` 实现了 `StaticDefault`, `OnceInit<T>` 才会实现 `Deref<Target = T>`.
    struct DefaultLogger;
    impl Logger for DefaultLogger {
        fn log(&self, _msg: &str) {
            // do nothing.
        }
    }
    unsafe impl StaticDefault for dyn Logger {
        fn static_default() -> &'static Self {
            static NOP: DefaultLogger = DefaultLogger;
            &NOP
        }
    }
}
mod a_logger {
    extern crate alloc;
    use crate::OnceInitError;
    // 一个简单的 a_logger crate.
    use super::log::{Logger, LOGGER};
    pub struct ALogger;

    impl Logger for ALogger {
        fn log(&self, msg: &str) {
            println!("{msg}");
        }
    }

    impl ALogger {
        pub fn init() -> Result<(), OnceInitError> {
            LOGGER.init_boxed(Box::new(ALogger))
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
