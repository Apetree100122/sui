// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[macro_export]
macro_rules! fatal {
    ($($arg:tt)*) => {{
        tracing::error!(fatal = true, $($arg)*);
        panic!($($arg)*);
    }};
}

#[macro_export]
macro_rules! debug_fatal {
    ($($arg:tt)*) => {{
        if cfg!(debug_assertions) {
            $crate::fatal!($($arg)*);
        } else {
            let stacktrace = std::backtrace::Backtrace::capture();
            tracing::error!(debug_fatal = true, stacktrace = ?stacktrace, $($arg)*);
            let location = concat!(file!(), ':', line!());
            if let Some(metrics) = mysten_metrics::get_metrics() {
                metrics.system_invariant_violations.with_label_values(&[location]).inc();
            }
        }
    }};
}

use futures::{pin_mut, FutureExt, Stream};
use std::io::Write;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct StructuredLog<T, W> {
    sender: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    _phantom: PhantomData<T>,
    writer_handle: std::thread::JoinHandle<W>,
}

impl<T: std::marker::Sized + serde::Serialize, W: 'static + Write + Send> StructuredLog<T, W> {
    pub fn new(mut writer: W) -> Self {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

        let writer_handle = std::thread::spawn(move || {
            while let Some(bytes) = receiver.blocking_recv() {
                writer
                    .write_all(&(bytes.len() as u32).to_le_bytes())
                    .unwrap();
                writer.write_all(&bytes).unwrap();
                writer.flush().unwrap();
            }
            writer
        });

        Self {
            sender,
            _phantom: PhantomData,
            writer_handle,
        }
    }

    pub fn into_writer(self) -> W {
        let Self {
            writer_handle,
            sender,
            ..
        } = self;
        drop(sender);
        writer_handle.join().unwrap()
    }

    pub fn write(&mut self, record: &T) -> std::io::Result<()> {
        let bytes = bcs::to_bytes(record)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        self.sender
            .send(bytes)
            .expect("Writer thread has terminated");

        Ok(())
    }
}

pub struct StructuredLogReader<T, R> {
    reader: R,
    _phantom: PhantomData<T>,
}

impl<T: std::marker::Sized, R: std::io::Read> StructuredLogReader<T, R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            _phantom: PhantomData,
        }
    }
}

impl<T: serde::de::DeserializeOwned, R: std::io::Read> Iterator for StructuredLogReader<T, R> {
    type Item = std::io::Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        // Read length prefix
        let mut len_buf = [0u8; 4];
        match self.reader.read_exact(&mut len_buf) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
            Err(e) => return Some(Err(e)),
        }

        let len = u32::from_le_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];

        // Read the actual data
        match self.reader.read_exact(&mut data) {
            Ok(()) => (),
            Err(e) => return Some(Err(e)),
        }

        // Parse the data
        let parsed = bcs::from_bytes::<T>(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e));

        Some(parsed)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[should_panic]
    fn test_fatal() {
        fatal!("This is a fatal error");
    }

    #[test]
    #[should_panic]
    fn test_debug_fatal() {
        if cfg!(debug_assertions) {
            debug_fatal!("This is a debug fatal error");
        } else {
            // pass in release mode as well
            fatal!("This is a fatal error");
        }
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn test_debug_fatal_release_mode() {
        debug_fatal!("This is a debug fatal error");
    }

    use futures::StreamExt;
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestStruct {
        field1: String,
        field2: i32,
    }

    #[test]
    fn test_structured_log_writer_reader() {
        let mut writer = StructuredLog::new(Vec::new());

        // Test writing multiple records
        let test_data = vec![
            TestStruct {
                field1: "test1".to_string(),
                field2: 42,
            },
            TestStruct {
                field1: "test2adfadf".to_string(),
                field2: 100,
            },
        ];

        for data in &test_data {
            writer.write(data).unwrap();
        }

        // Create reader from written data
        let cursor = Cursor::new(writer.into_writer());
        let reader = StructuredLogReader::<TestStruct, _>::new(cursor);

        // Test reading records
        let read_data: Vec<_> = reader.map(Result::unwrap).collect();
        assert_eq!(test_data, read_data);
    }

    #[test]
    fn test_structured_log_empty() {
        let writer = StructuredLog::<TestStruct, Vec<u8>>::new(Vec::new());

        // Create reader from empty buffer
        let cursor = Cursor::new(writer.into_writer());
        let reader = StructuredLogReader::<TestStruct, _>::new(cursor);

        // Should return no items for empty buffer
        assert_eq!(reader.count(), 0);
    }
}
