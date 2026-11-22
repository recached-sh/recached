samples: 35978, user cycles/op 23981

### Stack contains (inclusive)

| share | cycles/op | label |
|---:|---:|---|
|  12.1%   2906.8 | `decode (all)` |
|   0.0%      0.0 | `decode: general Value path (Args::general)` |
|   0.0%      0.0 | `decode: from_value` |
|   0.0%      0.0 | `decode: build (owned MSET/DEL)` |
|   0.0%      0.0 | `to_command (borrowed -> owned)` |
|   0.0%      0.0 | `borrowed execute (try_execute_borrowed)` |
|  70.5%  16907.4 | `ordered write entry (execute_ordered_write)` |
|  69.7%  16706.8 | `store execute (all)` |
|   0.0%      0.0 | `store: Command clone` |
|   7.5%   1800.3 | `store: key hash (sip)` |
|  35.9%   8613.7 | `store: entry_state` |
|   0.0%      0.0 | `store: extract_key` |
|   0.8%    192.0 | `clock reads` |
|   1.3%    305.9 | `metrics record` |
|   0.2%     51.3 | `reply serialize (Value)` |
|   0.1%     19.3 | `Value drop` |
|   0.1%     30.7 | `Command drop` |
|   3.5%    833.8 | `allocator (all)` |
|   1.9%    460.6 | `select overflow arm (mpsc recv)` |
|   3.6%    869.8 | `BufWriter flush (all)` |
|   3.9%    945.8 | `AofGatedWriter (all)` |
|   0.6%    141.3 | `DeferredAof pop` |
|   1.1%    255.3 | `socket read (poll_read_priv)` |
|   1.4%    341.3 | `socket write (poll_write_priv)` |
|   3.9%    945.8 | `split lock (ReadHalf/WriteHalf)` |
|   1.0%    228.0 | `tokio park/driver` |
|  98.5%  23619.4 | `tokio run_task (scheduler)` |
|   0.0%      0.0 | `task-local scope (with_deferred_aof)` |

### Breakdown: innermost context x leaf class (each sample once)

| context | leaf class | share | cycles/op |
|---|---|---:|---:|
| store execute | code |  49.0% |   11739 |
| store execute | libc (memcpy, syscall wrappers) |  18.5% |    4437 |
| store execute | allocator |   0.9% |     223 |
| **store execute** | **all** | ** 68.4%** | **  16399** |
| connection (decode, reply, socket) | code |  15.7% |    3755 |
| connection (decode, reply, socket) | allocator |   2.6% |     624 |
| connection (decode, reply, socket) | libc (memcpy, syscall wrappers) |   1.5% |     359 |
| connection (decode, reply, socket) | tokio mpsc |   1.2% |     287 |
| connection (decode, reply, socket) | tokio wake/schedule |   0.4% |     107 |
| **connection (decode, reply, socket)** | **all** | ** 21.4%** | **   5131** |
| tokio scheduler | tokio wake/schedule |   3.5% |     835 |
| tokio scheduler | code |   1.1% |     268 |
| tokio scheduler | tokio io/split |   0.6% |     149 |
| tokio scheduler | libc (memcpy, syscall wrappers) |   0.1% |      15 |
| tokio scheduler | allocator |   0.0% |       2 |
| **tokio scheduler** | **all** | **  5.3%** | **   1268** |
| AOF append/lead/gate | tokio io/split |   1.7% |     419 |
| AOF append/lead/gate | code |   1.4% |     328 |
| AOF append/lead/gate | libc (memcpy, syscall wrappers) |   0.8% |     201 |
| **AOF append/lead/gate** | **all** | **  4.0%** | **    948** |
| ordered write path (rest) | code |   0.8% |     201 |
| **ordered write path (rest)** | **all** | **  0.8%** | **    201** |
| tokio io driver | code |   0.1% |      25 |
| tokio io driver | libc (memcpy, syscall wrappers) |   0.0% |       9 |
| **tokio io driver** | **all** | **  0.1%** | **     34** |

### Self (leaf)

| share | cycles/op | function |
|---:|---:|---|
|  17.7%   4248.5 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  12.5%   2995.4 | `libc.so.6:+1c7919` |
|   5.5%   1319.7 | `<core_engine::request::RequestDecoder>::decode_any` |
|   4.1%    971.8 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   3.7%    886.5 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   3.5%    850.5 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|   3.5%    833.8 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   3.4%    827.2 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   3.3%    795.8 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|   2.4%    579.9 | `core_engine::store::entry_size` |
|   2.4%    564.6 | `core_engine::request::build` |
|   2.0%    490.6 | `_rjem_malloc` |
|   1.9%    458.6 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   1.6%    382.6 | `core::slice::sort::shared::smallsort::insertion_sort_shift_left::<&str, <&str as core::cmp::PartialOrd>::lt>` |
|   1.6%    381.3 | `core_engine::store::lock_stripe` |
|   1.3%    311.9 | `<core_engine::store::KeyValueStore>::entry_state` |
|   1.2%    293.9 | `_rjem_sdallocx` |
|   1.2%    289.3 | `libc.so.6:+1c79c7` |
|   1.2%    289.3 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   1.2%    284.6 | `core::str::converts::from_utf8` |
|   1.0%    238.0 | `core::ptr::drop_glue::<alloc::vec::Vec<std::sync::poison::mutex::MutexGuard<()>>>` |
|   1.0%    230.6 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|   0.9%    227.3 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|   0.9%    217.3 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|   0.8%    203.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   0.8%    195.3 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|   0.8%    188.6 | `libc.so.6:+1c791d` |
|   0.7%    168.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.7%    160.6 | `recached_server::clients::record_metrics` |
|   0.6%    148.6 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.6%    141.3 | `<recached_server::persistence::DeferredAof>::pop` |
|   0.4%    107.3 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   0.4%    106.6 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.4%    100.0 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|   0.4%     98.6 | `libc.so.6:+1c7620` |
|   0.4%     97.3 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   0.4%     92.0 | `[vdso]:+a8c` |
|   0.4%     90.6 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   0.4%     90.0 | `<core_engine::request::Args>::key_ref` |
|   0.4%     88.0 | `core_engine::store::write_slot::<&str>` |
|   0.4%     86.6 | `tokio::runtime::task::waker::clone_waker` |
|   0.4%     84.0 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   0.3%     78.0 | `core_engine::request::build_ref` |
|   0.3%     77.3 | `<smallvec::SmallVec<[&str; 2]> as core::iter::traits::collect::FromIterator<&str>>::from_iter::<core::iter::a…` |
|   0.3%     74.7 | `libc.so.6:+1c7708` |
|   0.3%     66.0 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.3%     62.7 | `core::ptr::drop_glue::<core_engine::store::EntryValue>` |
|   0.3%     60.0 | `core::ptr::drop_glue::<alloc::vec::Vec<(alloc::string::String, alloc::vec::Vec<u8>)>>` |
|   0.2%     59.3 | `[unknown]` |
|   0.2%     59.3 | `libc.so.6:+175818` |
|   0.2%     56.7 | `libc.so.6:+1c7900` |
|   0.2%     55.3 | `tokio::runtime::task::waker::wake_by_val` |
|   0.2%     55.3 | `<core_engine::store::SlotRef as core::ops::drop::Drop>::drop` |
|   0.2%     53.3 | `libc.so.6:+1c790e` |
|   0.2%     53.3 | `<smallvec::SmallVec<[std::sync::poison::mutex::MutexGuard<()>; 4]>>::try_grow` |
|   0.2%     52.7 | `<recached_server::persistence::AOF_DEFERRED::__KEY::{K#0}::{closure#0} as core::ops::function::FnOnce<(core::…` |
|   0.2%     52.7 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   0.2%     48.0 | `libc.so.6:+1c7928` |
|   0.2%     46.7 | `libc.so.6:+1c7d80` |
|   0.2%     46.0 | `<smallvec::SmallVec<[usize; 4]>>::try_grow` |

### Inclusive

| share | cycles/op | function |
|---:|---:|---|
| 100.0%  23980.7 | `tokio::runtime::task::raw::poll::<tokio::runtime::blocking::task::BlockingTask<<tokio::runtime::scheduler::mu…` |
|  98.5%  23619.4 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::run_task` |
|  97.5%  23387.5 | `tokio::runtime::task::raw::poll::<core::pin::Pin<alloc::boxed::Box<recached_server::run::{closure#0}::{closur…` |
|  96.6%  23169.5 | `recached_server::run::{closure#0}::{closure#27}::{closure#0}` |
|  95.4%  22884.9 | `recached_server::connection::handle_tcp::<tokio::net::tcp::stream::TcpStream>::{closure#0}::{closure#0}` |
|  70.5%  16902.1 | `recached_server::propagation::execute_ordered_write::{closure#0}` |
|  69.7%  16706.8 | `recached_server::clients::execute_and_record_ref` |
|  68.0%  16306.8 | `<core_engine::store::KeyValueStore>::execute_reporting_ref` |
|  37.2%   8924.3 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|  35.9%   8613.7 | `<core_engine::store::KeyValueStore>::entry_state` |
|  12.5%   2995.4 | `libc.so.6:+1c7919` |
|  12.1%   2906.8 | `<core_engine::request::RequestDecoder>::decode_any` |
|  10.4%   2482.2 | `<core_engine::store::KeyValueStore>::execute_inner_ref` |
|   7.4%   1783.7 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>>>::hash_u64::<&str>` |
|   6.3%   1504.4 | `core_engine::request::build` |
|   6.1%   1461.7 | `<core_engine::store::KeyValueStore>::order_keys::<&str>` |
|   5.4%   1303.1 | `core_engine::store::write_slot::<&str>` |
|   5.1%   1215.1 | `<dashmap::DashMap<alloc::sync::Arc<str>, alloc::boxed::Box<core_engine::store::Entry>> as dashmap::t::Map<all…` |
|   4.1%    971.8 | `<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write` |
|   3.6%    869.8 | `<tokio::io::util::flush::Flush<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedW…` |
|   3.4%    821.8 | `core::slice::sort::shared::smallsort::insertion_sort_shift_left::<&str, <&str as core::cmp::PartialOrd>::lt>` |
|   2.9%    691.2 | `<tokio::io::util::buf_writer::BufWriter<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteH…` |
|   2.5%    600.6 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   2.4%    579.9 | `core_engine::store::entry_size` |
|   2.0%    490.6 | `_rjem_malloc` |
|   1.9%    460.6 | `<tokio::sync::mpsc::bounded::Receiver<()>>::recv::{closure#0}` |
|   1.6%    381.3 | `core_engine::store::lock_stripe` |
|   1.6%    374.6 | `<core_engine::request::Args>::key_ref` |
|   1.4%    341.3 | `<tokio::net::tcp::stream::TcpStream>::poll_write_priv` |
|   1.3%    314.6 | `core::ptr::drop_glue::<alloc::vec::Vec<(alloc::string::String, alloc::vec::Vec<u8>)>>` |
|   1.3%    305.9 | `recached_server::clients::record_metrics` |
|   1.2%    293.9 | `_rjem_sdallocx` |
|   1.2%    289.3 | `libc.so.6:+1c79c7` |
|   1.2%    289.3 | `<core::iter::adapters::zip::Zip<core::iter::adapters::zip::Zip<core::slice::iter::Iter<&str>, smallvec::IntoI…` |
|   1.2%    284.6 | `core::str::converts::from_utf8` |
|   1.1%    255.3 | `<tokio::net::tcp::stream::TcpStream>::poll_read_priv` |
|   1.1%    252.6 | `core::ptr::drop_glue::<smallvec::SmallVec<[std::sync::poison::mutex::MutexGuard<()>; 4]>>` |
|   1.0%    238.0 | `core::ptr::drop_glue::<alloc::vec::Vec<std::sync::poison::mutex::MutexGuard<()>>>` |
|   1.0%    228.0 | `<tokio::runtime::scheduler::multi_thread::worker::Context>::park_internal` |
|   0.9%    212.0 | `<tokio::runtime::time::Driver>::park_internal` |
|   0.8%    203.3 | `<tokio::sync::mpsc::list::Rx<()>>::pop` |
|   0.8%    196.0 | `<tokio::runtime::io::driver::Driver>::turn` |
|   0.8%    194.0 | `<tokio::runtime::io::registration::Registration>::poll_ready` |
|   0.8%    192.0 | `<std::sys::pal::unix::time::Timespec>::now` |
|   0.8%    188.6 | `libc.so.6:+1c791d` |
|   0.7%    173.3 | `<tokio::sync::task::atomic_waker::AtomicWaker>::register_by_ref` |
|   0.7%    172.0 | `<recached_server::persistence::AofGatedWriter<tokio::io::split::WriteHalf<tokio::net::tcp::stream::TcpStream>…` |
|   0.7%    163.3 | `libc.so.6:+139b5b` |
|   0.7%    162.0 | `<tokio::runtime::io::scheduled_io::ScheduledIo>::wake` |
|   0.6%    153.3 | `core_engine::store::mutation_scope` |
|   0.6%    141.3 | `<recached_server::persistence::DeferredAof>::pop` |
|   0.6%    133.3 | `<smallvec::SmallVec<[&str; 2]> as core::iter::traits::collect::FromIterator<&str>>::from_iter::<core::iter::a…` |
|   0.5%    116.6 | `<alloc::raw_vec::RawVecInner>::try_allocate_in` |
|   0.5%    110.0 | `<smallvec::SmallVec<[usize; 4]>>::try_grow` |
|   0.4%    100.0 | `<std::sys::thread::unix::Thread>::new::thread_start` |
|   0.4%    100.0 | `libc.so.6:+edc42` |
|   0.4%    100.0 | `libc.so.6:+17319c` |
|   0.4%    100.0 | `<std::thread::lifecycle::spawn_unchecked<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::{closure#0}…` |
|   0.4%    100.0 | `std::sys::backtrace::__rust_begin_short_backtrace::<<tokio::runtime::blocking::pool::Spawner>::spawn_thread::…` |
|   0.4%     98.6 | `libc.so.6:+1c7620` |
