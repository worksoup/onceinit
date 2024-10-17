// MIT License
//
// Copyright (c) 2024 worksoup <https://github.com/worksoup/>
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

#![feature(sync_unsafe_cell)]
#![doc = include_str!("../README.md")]
use std::{
    cell::SyncUnsafeCell,
    ops::Deref,
    sync::atomic::{
        AtomicUsize,
        Ordering,
    },
};

#[derive(Debug, thiserror::Error)]
/// # `OnceInitError`
/// 读取或初始化 [`OnceInit`] 内部数据时可能返回该错误。
pub enum OnceInitError {
    /// 数据正在初始化。
    #[error("data is initializing.")]
    DataInitializing,
    /// 数据已被初始化。
    #[error("data has already been initialized.")]
    DataInitialized,
    /// 数据未被初始化。
    #[error("data is uninitialized.")]
    DataUninitialized,
}

const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;

/// # `OnceInit`
/// 仅可设置一次数据的类型。
pub struct OnceInit<T: ?Sized + 'static>
where
    &'static T: Sized,
{
    state: AtomicUsize,
    data: SyncUnsafeCell<Option<&'static T>>,
}
impl<T: ?Sized> OnceInit<T>
where
    &'static T: Sized,
    Self: Sized,
{
    /// 返回未初始化的 [`OnceInit`] 类型。
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(UNINITIALIZED),
            data: SyncUnsafeCell::new(None),
        }
    }
}

impl<T: ?Sized> OnceInit<T> {
    /// 不检查是否初始化，直接返回内部数据。
    ///
    /// # Safety
    ///
    /// 未初始化时，调用此函数会在内部的 [`None`] 值上调用 [`Option::unwrap_unchecked`], 造成[*未定义行为*]。
    ///
    /// [*未定义行为*]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    /// ```
    pub unsafe fn get_data_unchecked(&self) -> &'static T {
        unsafe { (*self.data.get()).unwrap_unchecked() }
    }
    /// 返回内部数据，若未初始化，则返回 [`OnceInitError`].
    pub fn get_data(&self) -> Result<&'static T, OnceInitError> {
        match self.state.load(Ordering::Acquire) {
            INITIALIZED => Ok(unsafe { (*self.data.get()).unwrap_unchecked() }),
            INITIALIZING => Err(OnceInitError::DataInitializing),
            _ => Err(OnceInitError::DataUninitialized),
        }
    }
    fn set_data_internal<F>(&self, make_data: F) -> Result<(), OnceInitError>
    where
        F: FnOnce() -> &'static T,
    {
        let old_state = match self.state.compare_exchange(
            UNINITIALIZED,
            INITIALIZING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(s) | Err(s) => s,
        };
        match old_state {
            INITIALIZING => {
                while self.state.load(Ordering::SeqCst) == INITIALIZING {
                    std::hint::spin_loop()
                }
                Err(OnceInitError::DataInitializing)
            }
            INITIALIZED => Err(OnceInitError::DataInitialized),
            _ => {
                unsafe { *self.data.get() = Some(make_data()) }
                self.state.store(INITIALIZED, Ordering::SeqCst);
                Ok(())
            }
        }
    }
    /// 设置内部数据，只可调用一次，成功则初始化完成，之后调用均会返回错误。
    pub fn set_data(&self, preprocessor: &'static T) -> Result<(), OnceInitError> {
        self.set_data_internal(|| preprocessor)
    }
    /// 设置内部数据，只可调用一次，成功则初始化完成，之后调用均会返回错误。
    pub fn set_boxed_data(&self, preprocessor: Box<T>) -> Result<(), OnceInitError> {
        self.set_data_internal(|| Box::leak(preprocessor))
    }
}
pub trait StaticDefault {
    fn static_default() -> &'static Self;
}
impl<T: ?Sized + StaticDefault> Deref for OnceInit<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get_data().unwrap_or_else(|_| T::static_default())
    }
}

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
