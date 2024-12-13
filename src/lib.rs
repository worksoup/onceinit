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

# ![doc = include_str!("../README.md")]
#![cfg_attr(feature = "no_std", no_std)]
#[cfg(all(not(feature = "no_std"), test))]
mod tests;

#[cfg(feature = "alloc")]
extern crate alloc;

use ::core::{
    cell::UnsafeCell,
    error::Error,
    fmt::Display,
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::fmt::Debug;

#[derive(Debug)]
/// # `OnceInitError`
/// 读取或初始化 [`OnceInit`] 内部数据时可能返回该错误。
pub enum OnceInitError {
    /// 数据未被初始化。
    DataUninitialized,
    /// 数据已被初始化。
    DataInitialized,
}

impl Display for OnceInitError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            OnceInitError::DataUninitialized {} => f.write_str("data is uninitialized."),
            OnceInitError::DataInitialized {} => f.write_str("data has already been initialized."),
        }
    }
}
impl Error for OnceInitError {}
#[derive(Debug)]
#[repr(usize)]
/// # `OnceInitState`
/// 表示 [`OnceInit`] 内部数据的初始化状态。
pub enum OnceInitState {
    /// 数据未被初始化。
    UNINITIALIZED = 0,
    /// 数据已被初始化。
    INITIALIZED = 2,
}

const UNINITIALIZED: usize = 0;
const INITIALIZING: usize = 1;
const INITIALIZED: usize = 2;

/// # `OnceInit`
/// 仅可设置一次数据的类型。
///
/// 当 `T` 实现了 [`Sync`] 时，该类型也会实现 [`Sync`].
/// [`Sync`] 是由内部原子类型的 `state` 和外部 api 共同保证的。
/// 外部 api 保证，当 `state` 指示数据正在或已经初始化时，该类型不可变。
pub struct OnceInit<T: ?Sized + 'static>
where
    &'static T: Sized,
{
    state: AtomicUsize,
    data: UnsafeCell<Option<&'static T>>,
}

impl<T: ?Sized> OnceInit<T> {
    pub const DEFAULT: Self = Self {
        state: AtomicUsize::new(UNINITIALIZED),
        data: UnsafeCell::new(None),
    };
    /// 返回未初始化的 [`OnceInit`] 类型。
    #[inline]
    pub const fn uninit() -> Self {
        Self::DEFAULT
    }
    /// 返回初始化过的 [`OnceInit`] 类型。
    #[inline]
    pub const fn new(data: &'static T) -> Self
    where
        &'static T: Sized,
        Self: Sized,
    {
        Self {
            state: AtomicUsize::new(INITIALIZED),
            data: UnsafeCell::new(Some(data)),
        }
    }
    /// 返回内部数据，若未初始化，则返回 [`OnceInitError`].
    ///
    /// 若需要可变数据，请在内部使用具有内部可见性的数据结构，如 [`Mutex`](std::sync::Mutex) 等。
    #[inline]
    pub fn get(&self) -> Result<&'static T, OnceInitError> {
        match self.state.load(Ordering::Acquire) {
            INITIALIZED => Ok(unsafe { (*self.data.get()).unwrap_unchecked() }),
            INITIALIZING => {
                while self.state.load(Ordering::SeqCst) == INITIALIZING {
                    core::hint::spin_loop()
                }
                Ok(unsafe { (*self.data.get()).unwrap_unchecked() })
            }
            _ => Err(OnceInitError::DataUninitialized),
        }
    }
    /// 返回内部数据，若未初始化，则返回 `<T as StaticDefault>::static_default()`.
    ///
    /// 需要 `T` 实现 [`StaticDefault`].
    #[inline]
    pub fn get_or_default(&self) -> &'static T
    where
        T: StaticDefault,
    {
        self.get().unwrap_or_else(|_| T::static_default())
    }
    /// 不检查是否初始化，直接返回内部数据。
    ///
    /// 若需要可变数据，请在内部使用具有内部可见性的数据结构，如 [`Mutex`](std::sync::Mutex) 等。
    ///
    /// # Safety
    ///
    /// 未初始化时，调用此函数会在内部的 [`None`] 值上调用 [`Option::unwrap_unchecked`], 造成[*未定义行为*]。
    ///
    /// [*未定义行为*]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[inline]
    pub unsafe fn get_unchecked(&self) -> &'static T {
        unsafe { (*self.data.get()).unwrap_unchecked() }
    }
    /// 返回数据状态，见 [`OnceInitState`].
    pub fn state(&self) -> OnceInitState {
        match self.state.load(Ordering::Acquire) {
            UNINITIALIZED => OnceInitState::UNINITIALIZED,
            INITIALIZING => {
                while self.state.load(Ordering::SeqCst) == INITIALIZING {
                    core::hint::spin_loop()
                }
                OnceInitState::UNINITIALIZED
            }
            INITIALIZED => OnceInitState::INITIALIZED,
            _ => unreachable!(),
        }
    }
    fn init_internal<F>(&self, make_data: F) -> Result<(), OnceInitError>
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
                    core::hint::spin_loop()
                }
                Err(OnceInitError::DataInitialized)
            }
            INITIALIZED => Err(OnceInitError::DataInitialized),
            _ => {
                unsafe { *self.data.get() = Some(make_data()) }
                self.state.store(INITIALIZED, Ordering::SeqCst);
                Ok(())
            }
        }
    }
    /// 初始化内部数据，只可调用一次，成功则初始化完成，之后调用均会返回错误。
    ///
    /// 如果 `data` 不是 `'static` 的，请使用 [`init_boxed`](Self::init_boxed).
    #[inline]
    pub fn init(&self, data: &'static T) -> Result<(), OnceInitError> {
        self.init_internal(|| data)
    }
    /// 初始化内部数据，只可调用一次，成功则初始化完成，之后调用均会返回错误。
    #[inline]
    #[cfg(any(feature = "alloc", not(feature = "no_std")))]
    pub fn init_boxed(&self, data: Box<T>) -> Result<(), OnceInitError> {
        self.init_internal(|| Box::leak(data))
    }
}
unsafe impl<T> Sync for OnceInit<T> where T: ?Sized + Sync {}
impl<T: ?Sized> Default for OnceInit<T>
where
    T: Sized + StaticDefault,
    Self: Sized,
{
    #[inline]
    fn default() -> Self {
        Self::new(T::static_default())
    }
}
impl<T: ?Sized + Debug> Debug for OnceInit<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_tuple("OnceInit");
        match self.get().ok() {
            Some(data) => d.field(&data),
            None => d.field(&format_args!("<uninit>")),
        };
        d.finish()
    }
}

/// # [`StaticDefault`]
///
/// 返回类型的 `'static` 生命周期引用。
///
/// ## Safety
///
/// 在实现该类型时，应当避免使用 [`Box::leak`], 这是因为该特型专为 [`OnceInit`] 设计，
/// 且 `OnceInit` **不保证** [`static_default`](StaticDefault::static_default) 只被调用一次。
///
/// 若内部使用了 `Box::leak`, 则可能会造成大量内存泄漏。
///
/// 最好只为真正拥有静态变量的类型实现该特型。
/// 如需使用 `Box::leak`, 请记得[初始化 `OnceInit`](OnceInit::init),
/// 初始化后的 `OnceInit` 将不再调用 `static_default`.
pub unsafe trait StaticDefault {
    /// 返回类型的 `'static` 生命周期引用。
    fn static_default() -> &'static Self;
}
impl<T: ?Sized + StaticDefault> Deref for OnceInit<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &'static Self::Target {
        self.get_or_default()
    }
}
/// 指示拥有一个全局实例，但可能未初始化。
pub trait UninitGlobalHolder<T: ?Sized> {
    /// 初始化内部数据。
    fn init(&self, data: &'static T) -> Result<(), OnceInitError>;
    /// 初始化内部数据。
    #[cfg(any(feature = "alloc", not(feature = "no_std")))]
    fn init_boxed(&self, data: Box<T>) -> Result<(), OnceInitError>;
}
impl<T: ?Sized> UninitGlobalHolder<T> for OnceInit<T> {
    /// 初始化内部数据，只可调用一次，成功则初始化完成，之后调用均会返回错误。
    ///
    /// 如果 `data` 不是 `'static` 的，请使用 [`init_boxed`](Self::init_boxed).
    #[inline]
    fn init(&self, data: &'static T) -> Result<(), OnceInitError> {
        OnceInit::init(self, data)
    }
    /// 初始化内部数据，只可调用一次，成功则初始化完成，之后调用均会返回错误。
    #[inline]
    fn init_boxed(&self, data: Box<T>) -> Result<(), OnceInitError> {
        OnceInit::init_boxed(self, data)
    }
}
/// 一个可能有用的模式。
///
/// 该模式表示：类型 `T` 拥有一个全局实例，并被 `M` 包装，可以对其进行初始化。
pub trait UninitGlobal<T: ?Sized, M: UninitGlobalHolder<T>> {
    fn holder() -> &'static M;
    #[inline]
    fn init(data: &'static T) -> Result<(), OnceInitError>
    where
        M: 'static,
    {
        Ok(Self::holder().init(data)?)
    }
    #[inline]
    #[cfg(any(feature = "alloc", not(feature = "no_std")))]
    fn init_boxed(data: Box<T>) -> Result<(), OnceInitError>
    where
        M: 'static,
    {
        Ok(Self::holder().init_boxed(data)?)
    }
}
