samples: 24371, user cycles/op 10195

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  22.3%   2277.5 | `decode (all)` |
|  14.3%   1459.2 | `decode: general Value path (Args::general)` |
|  11.2%   1146.3 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  40.8%   4162.5 | `ordered write entry (execute_ordered_write)` |
|  39.2%   3992.3 | `store execute (all)` |
|   3.1%    314.2 | `store: Command clone` |
|   2.5%    259.8 | `store: key hash (sip)` |
|   3.1%    314.2 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   3.1%    317.9 | `clock reads` |
|   3.2%    323.4 | `metrics record` |
|   0.1%     13.4 | `reply serialize (Value)` |
|   1.0%     99.1 | `Value drop` |
|   0.3%     35.1 | `Command drop` |
|   4.8%    489.0 | `allocator (all)` |
|   4.2%    432.6 | `select overflow arm (mpsc recv)` |
|   8.1%    827.5 | `BufWriter flush (all)` |
|   9.2%    932.9 | `AofGatedWriter (all)` |
|   1.4%    147.3 | `DeferredAof pop` |
|   2.5%    253.9 | `socket read (poll_read_priv)` |
|   2.4%    239.7 | `socket write (poll_write_priv)` |
|   9.2%    933.3 | `split lock (ReadHalf/WriteHalf)` |
|   2.2%    220.9 | `tokio park/driver` |
|  96.7%   9859.1 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| connection (decode, reply, socket) | code |  32.4% |    3307 |
| connection (decode, reply, socket) | allocator |   4.0% |     406 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   2.7% |     278 |
| connection (decode, reply, socket) | tokio mpsc |   2.4% |     243 |
| connection (decode, reply, socket) | tokio wake/schedule |   1.1% |     117 |
| **connection (decode, reply, socket)** | **all** | ** 42.7%** | **   4351** |
| store execute | code |  28.3% |    2887 |
| store execute | libc (memcpy, syscall wrappers) |   6.0% |     616 |
| store execute | allocator |   0.8% |      82 |
| **store execute** | **all** | ** 35.2%** | **   3584** |
| tokio scheduler | tokio wake/schedule |   7.2% |     736 |
| tokio scheduler | code |   2.3% |     236 |
| tokio scheduler | tokio io/split |   1.2% |     127 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.2% |      15 |
| tokio scheduler | allocator |   0.0% |       1 |
| **tokio scheduler** | **all** | ** 10.9%** | **   1116** |
| AOF append/lead/gate | tokio io/split |   5.1% |     520 |
| AOF append/lead/gate | code |   2.8% |     284 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   1.3% |     132 |
| **AOF append/lead/gate** | **all** | **  9.2%** | **    935** |
| ordered write path (rest) | code |   1.7% |     170 |
| **ordered write path (rest)** | **all** | **  1.7%** | **    170** |
| tokio io driver | code |   0.3% |      33 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.1% |       6 |
| **tokio io driver** | **all** | **  0.4%** | **     39** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  10.2%   1041.3 | `<indexmap::map::IndexMap<alloc::string::String, ()>>::insert_full` |
|   8.7%    890.7 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   5.5%    562.3 | `<core_engine::cmd::Command>::from_value` |
|   4.8%    490.3 | `<core_engine::request::RequestDecoder>::decode_any` |
|   3.2%    326.7 | `libc.so.6:+1c7919` |
|   3.1%    313.8 | `_rjem_malloc` |
|   2.9%    292.4 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   2.0%    202.1 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   2.0%    200.0 | `core_engine::request::build_ref` |
|   1.9%    198.7 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.8%    188.3 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.8%    182.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.7%    171.5 | `recached_server::clients::record_metrics` |
|   1.6%    163.6 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   1.6%    162.3 | `_rjem_sdallocx` |
|   1.5%    156.5 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.5%    149.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.4%    147.3 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.3%    132.6 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.3%    129.3 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   1.2%    127.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.2%    126.8 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.2%    125.1 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.2%    120.1 | `core_engine::cmd::take_string` |
|   1.1%    117.1 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.0%    105.8 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   1.0%    100.8 | `[vdso]:+a8c` |
|   1.0%     99.1 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   1.0%     97.9 | `libc.so.6:+1c7d91` |
|   1.0%     97.5 | `<core_engine::store::KeyValueStore>::execute_inner` |
|   0.9%     93.7 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.9%     92.5 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   0.9%     92.0 | `core_engine::request::build` |
|   0.9%     88.7 | `<core_engine::request::Args>::general` |
|   0.9%     87.9 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   0.8%     86.2 | `core::str::converts::from_utf8` |
|   0.8%     84.5 | `recached_server::clients::command_name` |
|   0.8%     83.3 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   0.8%     82.8 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.8%     79.5 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   0.8%     76.6 | `core::ptr::drop_glue::<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<…` |
|   0.7%     71.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   0.7%     68.6 | `<alloc::string::String as core::clone::Clone>::clone` |
|   0.7%     67.8 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.7%     67.8 | `tokio::runtime::task::waker::clone_waker` |
|   0.6%     65.3 | `<core::iter::adapters::filter::Filter<core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<alloc::s…` |
|   0.6%     59.0 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.6%     58.6 | `[unknown]` |
|   0.5%     53.1 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&alloc::string::String>` |
|   0.5%     52.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.5%     51.9 | `libc.so.6:+175818` |
|   0.5%     51.0 | `core_engine::store::lock_stripe` |
|   0.5%     50.2 | `tokio::runtime::task::waker::wake_by_val` |
|   0.5%     49.4 | `libc.so.6:+139b40` |
|   0.4%     44.3 | `<str>::to_uppercase` |
|   0.4%     43.9 | `<alloc::vec::Vec<alloc::string::String> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<alloc::strin…` |
|   0.4%     43.1 | `core_engine::store::write_slot::<alloc::string::String>` |
|   0.4%     41.4 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   0.4%     36.0 | `core_engine::store::entry_size` |
|   0.3%     35.6 | `recached_server::clients::execute_and_record_ref` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  10195.5 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  96.7%   9859.1 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  94.7%   9659.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  92.9%   9471.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  90.7%   9244.2 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  40.8%   4155.8 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  39.2%   3992.3 | `recached_server::clients::execute_and_record_ref` |
|  33.2%   3383.2 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  22.3%   2277.5 | `<core_engine::request::RequestDecoder>::decode_any` |
|  19.3%   1971.7 | `<core_engine::store::KeyValueStore>::execute_inner` |
|  15.2%   1551.6 | `<core::iter::adapters::filter::Filter<core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<alloc::s…` |
|  14.5%   1482.2 | `<core_engine::store::CompactSet>::insert` |
|  14.4%   1464.2 | `<indexmap::map::IndexMap<alloc::string::String, ()>>::insert_full` |
|  14.3%   1459.2 | `<core_engine::request::Args>::general` |
|  11.2%   1146.3 | `<core_engine::cmd::Command>::from_value` |
|   8.1%    827.5 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   6.0%    609.1 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   4.7%    484.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   4.2%    432.6 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   3.4%    351.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   3.2%    326.7 | `libc.so.6:+1c7919` |
|   3.2%    323.4 | `recached_server::clients::record_metrics` |
|   3.1%    317.9 | `<std::sys::pal::unix::time::Timespec>::now` |
|   3.1%    314.2 | `<core_engine::cmd::Command as core::clone::Clone>::clone` |
|   3.1%    314.2 | `<core_engine::store::KeyValueStore>::entry_state` |
|   3.1%    313.8 | `_rjem_malloc` |
|   2.5%    253.9 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   2.4%    239.7 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   2.2%    220.9 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   2.0%    202.9 | `<tokio::runtime::time::Driver>::park_internal` |
|   2.0%    200.4 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.0%    200.0 | `core_engine::request::build_ref` |
|   1.9%    197.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   1.9%    189.5 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   1.8%    187.4 | `<tokio::runtime::io::driver::Driver>::turn` |
|   1.8%    186.2 | `libc.so.6:+139b5b` |
|   1.8%    184.1 | `core_engine::store::write_slot::<alloc::string::String>` |
|   1.6%    162.3 | `_rjem_sdallocx` |
|   1.6%    160.6 | `core_engine::cmd::take_string` |
|   1.5%    154.0 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   1.5%    149.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   1.5%    148.5 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   1.5%    148.5 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   1.4%    147.3 | `core_engine::request::build` |
|   1.4%    147.3 | `<recached_server::persistence::DeferredAof>::pop` |
|   1.4%    146.4 | `core_engine::cmd::take_key` |
|   1.4%    141.0 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.4%    139.7 | `<alloc::vec::Vec<alloc::string::String> as alloc::vec::spec_from_iter_nested::SpecFromIterNested<alloc::strin…` |
|   1.3%    132.6 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   1.1%    107.9 | `<alloc::string::String as core::clone::Clone>::clone` |
|   1.0%    105.4 | `<tokio::io::util::write_all::WriteAll<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::Ao…` |
|   1.0%    100.8 | `[vdso]:+a8c` |
|   1.0%     99.1 | `core::ptr::drop_glue::<core_engine::resp::Value>` |
|   1.0%     97.9 | `libc.so.6:+1c7d91` |
|   0.9%     88.3 | `core_engine::store::now_ms` |
|   0.9%     87.9 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   0.9%     87.9 | `core::ptr::drop_glue::<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<…` |
|   0.8%     86.6 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&alloc::string::String>` |
|   0.8%     86.2 | `core::str::converts::from_utf8` |
|   0.8%     84.5 | `recached_server::clients::command_name` |
