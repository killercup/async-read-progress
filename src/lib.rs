//! Report the progress of an async read operation
//!
//! As promised [on Twitter](https://twitter.com/killercup/status/1254695847796842498).
//!
//! # Examples
//!
//! ```
//! # fn main() {
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! use futures::{
//!     io::AsyncReadExt,
//!     stream::{self, TryStreamExt},
//! };
//! use async_read_progress::*;
//!
//! let src = vec![1u8, 2, 3, 4, 5];
//! let total_size = src.len();
//! let reader = stream::iter(vec![Ok(src)]).into_async_read();
//!
//! let mut reader = reader.report_progress(
//!     /* only call every */ std::time::Duration::from_millis(20),
//!     |bytes_read| eprintln!("read {}/{}", bytes_read, total_size),
//! );
//! #
//! # let mut buf = Vec::new();
//! # assert!(reader.read_to_end(&mut buf).await.is_ok());
//! # });
//! # }
//! ```

pub use for_futures::FReportReadProgress as AsyncReadProgressExt;

#[cfg(feature = "with-tokio")]
pub use for_tokio::TReportReadProgress as TokioAsyncReadProgressExt;

/// Reader for the `report_progress` method.
#[must_use = "streams do nothing unless polled"]
pub struct LogStreamProgress<St, F> {
    inner: St,
    callback: F,
    bytes_read: usize,
    // TODO: Actually use this
    at_most_ever: std::time::Duration,
}

// TODO: Remove this comment after someone who knows how this actually works has
// reviewed/fixed this.
impl<St, F> LogStreamProgress<St, F> {
    pin_utils::unsafe_pinned!(inner: St);
    pin_utils::unsafe_unpinned!(callback: F);
    pin_utils::unsafe_unpinned!(bytes_read: usize);
}

impl<T, U> Unpin for LogStreamProgress<T, U>
where
    T: Unpin,
    U: Unpin,
{
}

mod for_futures {
    use core::{
        pin::Pin,
        task::{Context, Poll},
    };
    use futures_io::{AsyncRead as FAsyncRead, IoSliceMut};
    use std::{io, time::Duration};

    /// An extension trait which adds the `report_progress` method to
    /// `AsyncRead` types.
    ///
    /// Note: This is for [`futures_io::AsyncRead`].
    pub trait FReportReadProgress {
        fn report_progress<F>(
            self,
            at_most_ever: Duration,
            callback: F,
        ) -> super::LogStreamProgress<Self, F>
        where
            Self: Sized,
            F: FnMut(usize),
        {
            super::LogStreamProgress {
                inner: self,
                callback,
                bytes_read: 0,
                at_most_ever,
            }
        }
    }

    impl<R: FAsyncRead + ?Sized> FReportReadProgress for R {}

    impl<'a, St, F> FAsyncRead for super::LogStreamProgress<St, F>
    where
        St: FAsyncRead,
        F: FnMut(usize),
    {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            match self.as_mut().inner().poll_read(cx, buf) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(bytes_read)) => {
                    *self.as_mut().bytes_read() += bytes_read;
                    let read = self.as_ref().bytes_read;
                    (self.as_mut().callback())(read);
                    Poll::Ready(Ok(bytes_read))
                }
            }
        }

        fn poll_read_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &mut [IoSliceMut<'_>],
        ) -> Poll<io::Result<usize>> {
            match self.as_mut().inner().poll_read_vectored(cx, bufs) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(bytes_read)) => {
                    *self.as_mut().bytes_read() += bytes_read;
                    let read = self.as_ref().bytes_read;
                    (self.as_mut().callback())(read);
                    Poll::Ready(Ok(bytes_read))
                }
            }
        }
    }
}

#[cfg(feature = "with-tokio")]
mod for_tokio {
    use core::{
        pin::Pin,
        task::{Context, Poll},
    };
    use std::{io, time::Duration};
    use tokio::io::AsyncRead as TAsyncRead;

    /// An extension trait which adds the `report_progress` method to
    /// `AsyncRead` types.
    ///
    /// Note: This is for [`tokio::io::AsyncRead`].
    pub trait TReportReadProgress {
        fn report_progress<F>(
            self,
            at_most_ever: Duration,
            callback: F,
        ) -> super::LogStreamProgress<Self, F>
        where
            Self: Sized,
            F: FnMut(usize),
        {
            super::LogStreamProgress {
                inner: self,
                callback,
                bytes_read: 0,
                at_most_ever,
            }
        }
    }

    impl<R: TAsyncRead + ?Sized> TReportReadProgress for R {}

    impl<'a, St, F> TAsyncRead for super::LogStreamProgress<St, F>
    where
        St: TAsyncRead,
        F: FnMut(usize),
    {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            match self.as_mut().inner().poll_read(cx, buf) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(bytes_read)) => {
                    *self.as_mut().bytes_read() += bytes_read;
                    let read = self.as_ref().bytes_read;
                    (self.as_mut().callback())(read);
                    Poll::Ready(Ok(bytes_read))
                }
            }
        }
    }

    #[test]
    fn works_with_tokios_async_read() {
        use bytes::Bytes;
        use tokio::io::{stream_reader, AsyncReadExt};

        let src = vec![1u8, 2, 3, 4, 5];
        let total_size = src.len();
        let xs = tokio::stream::iter(vec![Ok(Bytes::from(src))]);
        let reader = stream_reader(xs);
        let mut buf = Vec::new();

        let mut reader = reader.report_progress(
            /* only call every */ Duration::from_millis(20),
            |bytes_read| eprintln!("read {}/{}", bytes_read, total_size),
        );

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            assert!(reader.read_to_end(&mut buf).await.is_ok());
        });
    }
}
