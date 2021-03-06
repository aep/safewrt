use std::io;
use std::time::{Duration, Instant};
use tokio_core::reactor::Timeout;
use futures::{Future, Poll};
use tokio_core::reactor::Handle;
use futures::{Async};

use tokio_io::{AsyncRead, AsyncWrite};

/// A future which will copy all data from a reader into a writer.
///
/// Created by the [`copy`] function, this future will resolve to the number of
/// bytes copied or an error if one happens.
///
/// [`copy`]: fn.copy.html
#[derive(Debug)]
pub struct CopyWithDeadline<R, W> {
    deadline: Duration,
    timeout:  Option<Timeout>,

    reader: Option<R>,
    read_done: bool,
    writer: Option<W>,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Box<[u8]>,
}

/// Creates a future which represents copying all the bytes from one object to
/// another.
///
/// The returned future will copy all the bytes read from `reader` into the
/// `writer` specified. This future will only complete once the `reader` has hit
/// EOF and all bytes have been written to and flushed from the `writer`
/// provided.
///
/// On success the number of bytes is returned and the `reader` and `writer` are
/// consumed. On error the error is returned and the I/O objects are consumed as
/// well.
pub fn copy_with_deadline<R, W>(reader: R, writer: W, handle: Handle, deadline: Duration) -> CopyWithDeadline<R, W>
    where R: AsyncRead,
          W: AsyncWrite,
{
    CopyWithDeadline {
        deadline: deadline,
        timeout:  Timeout::new(deadline, &handle).ok(),
        reader: Some(reader),
        read_done: false,
        writer: Some(writer),
        amt: 0,
        pos: 0,
        cap: 0,
        buf: Box::new([0; 2048]),
    }
}

impl<R, W> Future for CopyWithDeadline<R, W>
    where R: AsyncRead,
          W: AsyncWrite,
{
    type Item = (u64, R, W);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(u64, R, W), io::Error> {

        match self.timeout.poll() {
            Ok(Async::Ready(_)) => {
                info!("deadline exceeded");
                let reader = self.reader.take().unwrap();
                let writer = self.writer.take().unwrap();
                return Ok((self.amt, reader, writer).into())
            },
            _ => {},
        };

        loop {

            // If our buffer is empty, then we need to read some data to
            // continue.
            if self.pos == self.cap && !self.read_done {
                let reader = self.reader.as_mut().unwrap();
                let n = try_nb!(reader.read(&mut self.buf));

                if let Some(timeout) = self.timeout.as_mut() {
                    timeout.reset(Instant::now() + self.deadline);
                }

                if n == 0 {
                    self.read_done = true;
                } else {
                    self.pos = 0;
                    self.cap = n;
                }
            }

            // If our buffer has some data, let's write it out!
            while self.pos < self.cap {
                let writer = self.writer.as_mut().unwrap();
                let i = try_nb!(writer.write(&self.buf[self.pos..self.cap]));

                if let Some(timeout) = self.timeout.as_mut() {
                    timeout.reset(Instant::now() + self.deadline);
                }

                if i == 0 {
                    return Err(io::Error::new(io::ErrorKind::WriteZero,
                                              "write zero byte into writer"));
                } else {
                    self.pos += i;
                    self.amt += i as u64;
                }
            }

            // If we've written al the data and we've seen EOF, flush out the
            // data and finish the transfer.
            // done with the entire transfer.
            if self.pos == self.cap && self.read_done {
                try_nb!(self.writer.as_mut().unwrap().flush());
                let reader = self.reader.take().unwrap();
                let writer = self.writer.take().unwrap();
                return Ok((self.amt, reader, writer).into())
            }
        }
    }
}
