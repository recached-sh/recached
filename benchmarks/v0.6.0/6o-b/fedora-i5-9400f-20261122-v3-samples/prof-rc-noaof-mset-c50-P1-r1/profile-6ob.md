samples: 31648, user cycles/op 22968

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  11.6%   2666.3 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  71.9%  16516.6 | `ordered write entry (execute_ordered_write)` |
|  71.1%  16332.3 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   7.5%   1725.8 | `store: key hash (sip)` |
|  36.2%   8303.7 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   1.1%    252.6 | `clock reads` |
|   1.4%    323.7 | `metrics record` |
|   0.2%     55.9 | `reply serialize (Value)` |
|   0.0%     10.2 | `Value drop` |
|   0.0%      8.0 | `Command drop` |
|   3.1%    711.9 | `allocator (all)` |
|   0.0%      0.0 | `select overflow arm (mpsc recv)` |
|   2.8%    638.6 | `BufWriter flush (all)` |
|   3.0%    679.3 | `AofGatedWriter (all)` |
|   0.3%     61.7 | `DeferredAof pop` |
|   1.1%    252.6 | `socket read (poll_read_priv)` |
|   0.9%    208.3 | `socket write (poll_write_priv)` |
|   3.0%    679.3 | `split lock (ReadHalf/WriteHalf)` |
|   1.5%    341.8 | `tokio park/driver` |
|  97.3%  22343.4 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| store execute | code |  49.4% |   11336 |
| store execute | libc (memcpy, syscall wrappers) |  19.4% |    4448 |
| store execute | allocator |   0.8% |     192 |
| **store execute** | **all** | ** 69.6%** | **  15976** |
| connection (decode, reply, socket) | code |  15.4% |    3544 |
| connection (decode, reply, socket) | allocator |   2.4% |     543 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   1.5% |     355 |
| **connection (decode, reply, socket)** | **all** | ** 19.3%** | **   4441** |
| tokio scheduler | tokio wake/schedule |   4.8% |    1091 |
| tokio scheduler | code |   1.6% |     366 |
| tokio scheduler | tokio io/split |   0.6% |     131 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.1% |      34 |
| tokio scheduler | allocator |   0.0% |       1 |
| **tokio scheduler** | **all** | **  7.1%** | **   1624** |
| AOF append/lead/gate | tokio io/split |   1.7% |     389 |
| AOF append/lead/gate | code |   0.8% |     195 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.5% |     107 |
| **AOF append/lead/gate** | **all** | **  3.0%** | **    691** |
| ordered write path (rest) | code |   0.8% |     184 |
| **ordered write path (rest)** | **all** | **  0.8%** | **    184** |
| tokio io driver | code |   0.2% |      44 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       7 |
| **tokio io driver** | **all** | **  0.2%** | **     51** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  17.8%   4083.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  13.2%   3020.4 | `libc.so.6:+1c7919` |
|   5.2%   1185.8 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.0%    913.0 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   3.7%    843.3 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   3.5%    815.0 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   3.5%    812.8 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   3.5%    804.8 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   3.5%    797.6 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   2.6%    588.6 | `core_engine::store::entry_size` |
|   2.0%    459.4 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.9%    437.6 | `core_engine::request::build` |
|   1.7%    384.6 | `_rjem_malloc` |
|   1.7%    380.3 | `core::slice::sort::shared::smallsort::insertion_sort_shift_left::<&str, <&str as core::cmp::PartialOrd>::lt>` |
|   1.5%    340.4 | `core_engine::store::lock_stripe` |
|   1.5%    333.1 | `core::str::converts::from_utf8` |
|   1.3%    290.3 | `_rjem_sdallocx` |
|   1.2%    285.2 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   1.2%    282.3 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   1.2%    281.6 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   1.2%    276.5 | `<core_engine::store::KeyValueStore>::entry_state` |
|   1.2%    268.5 | `libc.so.6:+1c79c7` |
|   1.1%    257.6 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   1.1%    256.2 | `core::ptr::drop_glue::<alloc::vec::Vec<std::sync::poison::mutex::MutexGuard<()>>>` |
|   1.0%    219.2 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   0.8%    192.3 | `libc.so.6:+1c791d` |
|   0.8%    177.8 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   0.6%    143.0 | `recached_server::clients::record_metrics` |
|   0.6%    131.4 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.5%    122.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.5%    121.9 | `[vdso]:+a8c` |
|   0.5%    111.8 | `core_engine::store::write_slot::<&str>` |
|   0.5%    103.8 | `libc.so.6:+1c7708` |
|   0.5%    103.8 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.4%     96.5 | `core_engine::request::build_ref` |
|   0.4%     92.2 | `tokio::runtime::task::waker::wake_by_val` |
|   0.4%     88.5 | `<core_engine::request::Args>::key_ref` |
|   0.4%     84.2 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   0.3%     78.4 | `libc.so.6:+1c7620` |
|   0.3%     74.7 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.3%     73.3 | `libc.so.6:+1c7da5` |
|   0.3%     68.9 | `<smallvec::SmallVec<[&str; 2]> as core::iter::traits::collect::FromIterator<&str>>::from_iter::<core::iter::a…` |
|   0.3%     66.0 | `tokio::runtime::task::waker::clone_waker` |
|   0.3%     65.3 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.3%     63.9 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.3%     61.7 | `<recached_server::persistence::DeferredAof>::pop` |
|   0.3%     61.7 | `core::ptr::drop_glue::<core_engine::store::EntryValue>` |
|   0.3%     59.5 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.3%     58.1 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.3%     58.1 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.2%     56.6 | `libc.so.6:+1c790e` |
|   0.2%     55.2 | `[unknown]` |
|   0.2%     52.3 | `libc.so.6:+1c7900` |
|   0.2%     48.6 | `libc.so.6:+1c7d80` |
|   0.2%     47.2 | `libc.so.6:+175818` |
|   0.2%     45.7 | `libc.so.6:+1c7928` |
|   0.2%     45.7 | `<core_engine::request::Args>::name_is` |
|   0.2%     43.5 | `<core_engine::resp::Value>::serialize_into` |
|   0.2%     43.5 | `core::ptr::drop_glue::<alloc::vec::Vec<(alloc::string::String, alloc::vec::Vec<u8>)>>` |
|   0.2%     43.5 | `<smallvec::SmallVec<[usize; 4]>>::try_grow` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  22967.5 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  97.3%  22343.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  96.0%  22059.7 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  94.9%  21802.0 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  93.5%  21480.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  71.9%  16510.1 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  71.1%  16332.3 | `recached_server::clients::execute_and_record_ref` |
|  69.0%  15852.6 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  37.8%   8678.9 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  36.2%   8303.7 | `<core_engine::store::KeyValueStore>::entry_state` |
|  13.2%   3020.4 | `libc.so.6:+1c7919` |
|  11.6%   2666.3 | `<core_engine::request::RequestDecoder>::decode_any` |
|  10.7%   2455.1 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   7.4%   1693.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   6.1%   1396.3 | `core_engine::request::build` |
|   5.9%   1352.7 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   5.9%   1345.5 | `core_engine::store::write_slot::<&str>` |
|   5.4%   1233.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.0%    913.0 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   3.4%    783.1 | `core::slice::sort::shared::smallsort::insertion_sort_shift_left::<&str, <&str as core::cmp::PartialOrd>::lt>` |
|   2.8%    638.6 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   2.6%    588.6 | `core_engine::store::entry_size` |
|   2.1%    488.4 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   1.8%    421.6 | `<core_engine::request::Args>::key_ref` |
|   1.7%    384.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   1.7%    384.6 | `_rjem_malloc` |
|   1.5%    341.8 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   1.5%    340.4 | `core_engine::store::lock_stripe` |
|   1.5%    333.1 | `core::str::converts::from_utf8` |
|   1.4%    323.7 | `recached_server::clients::record_metrics` |
|   1.3%    290.3 | `_rjem_sdallocx` |
|   1.2%    285.2 | `core::ptr::drop_glue::<alloc::vec::Vec<(alloc::string::String, alloc::vec::Vec<u8>)>>` |
|   1.2%    281.6 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   1.2%    272.9 | `core::ptr::drop_glue::<smallvec::SmallVec<[std::sync::poison::mutex::MutexGuard<()>; 4]>>` |
|   1.2%    268.5 | `libc.so.6:+1c79c7` |
|   1.1%    256.2 | `core::ptr::drop_glue::<alloc::vec::Vec<std::sync::poison::mutex::MutexGuard<()>>>` |
|   1.1%    252.6 | `<std::sys::pal::unix::time::Timespec>::now` |
|   1.1%    252.6 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   1.0%    233.7 | `<tokio::runtime::time::Driver>::park_internal` |
|   1.0%    219.2 | `libc.so.6:+edc42` |
|   1.0%    219.2 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   1.0%    219.2 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   1.0%    219.2 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   1.0%    219.2 | `libc.so.6:+17319c` |
|   0.9%    208.3 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.9%    200.3 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.8%    195.2 | `libc.so.6:+139b5b` |
|   0.8%    192.3 | `libc.so.6:+1c791d` |
|   0.7%    169.1 | `tokio::runtime::task::waker::wake_by_val` |
|   0.7%    163.3 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.7%    149.5 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.6%    133.5 | `core_engine::store::mutation_scope` |
|   0.5%    121.9 | `[vdso]:+a8c` |
|   0.5%    119.7 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.5%    110.3 | `<smallvec::SmallVec<[&str; 2]> as core::iter::traits::collect::FromIterator<&str>>::from_iter::<core::iter::a…` |
|   0.5%    103.8 | `libc.so.6:+1c7708` |
|   0.4%     96.5 | `core_engine::request::build_ref` |
|   0.4%     90.7 | `<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>` |
|   0.4%     87.1 | `recached_server::reply_coalesce::before_flush::{closure#0}` |
|   0.4%     84.9 | `<smallvec::SmallVec<[std::sync::poison::mutex::MutexGuard<()>; 4]>>::try_grow` |
