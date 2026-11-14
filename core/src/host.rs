//! How the backend talks to the app that hosts it.
//!
//! The backend reaches "the app" through `AppHandle`: it stores shared state (`manage` /
//! `state`), emits events for the UI (`emit`) and listens to its own events (`listen`), all over
//! an in-process bus. The UI subscribes to every event with `listen_any` and calls the functions
//! in `commands` directly. `async_runtime` is the one tokio runtime for the whole process.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Other(String),
    #[error("serialize: {0}")]
    Serialize(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

/// One emitted event, as a listener sees it.
#[derive(Clone, Debug)]
pub struct Event {
    id: EventId,
    name: String,
    payload: String,
}
impl Event {
    pub fn id(&self) -> EventId {
        self.id
    }
    pub fn event(&self) -> &str {
        &self.name
    }
    /// The payload as JSON text.
    pub fn payload(&self) -> &str {
        &self.payload
    }
}
pub type EventId = u32;

type Handler = Arc<dyn Fn(Event) + Send + Sync>;

struct Inner {
    states: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
    listeners: Mutex<Vec<(EventId, Option<String>, Handler)>>,
    next_id: AtomicU32,
    exit: Mutex<Option<Box<dyn Fn(i32) + Send + Sync>>>,
}

/// The backend's handle on the host. Cheap to clone.
#[derive(Clone)]
pub struct AppHandle(Arc<Inner>);

impl Default for AppHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl AppHandle {
    pub fn new() -> Self {
        AppHandle(Arc::new(Inner {
            states: RwLock::new(HashMap::new()),
            listeners: Mutex::new(Vec::new()),
            next_id: AtomicU32::new(1),
            exit: Mutex::new(None),
        }))
    }

    /// Subscribe to every event (how the UI hears the backend).
    pub fn listen_any<F: Fn(Event) + Send + Sync + 'static>(&self, handler: F) -> EventId {
        let id = self.0.next_id.fetch_add(1, Ordering::Relaxed);
        self.0.listeners.lock().unwrap().push((id, None, Arc::new(handler)));
        id
    }

    /// What `exit()` does; the native UI installs this (e.g. close the window, end the loop).
    pub fn on_exit<F: Fn(i32) + Send + Sync + 'static>(&self, f: F) {
        *self.0.exit.lock().unwrap() = Some(Box::new(f));
    }

    pub fn exit(&self, code: i32) {
        if let Some(f) = self.0.exit.lock().unwrap().as_ref() {
            f(code);
        } else {
            std::process::exit(code);
        }
    }

    fn dispatch(&self, name: &str, payload: String) {
        let id = self.0.next_id.fetch_add(1, Ordering::Relaxed);
        let ev = Event { id, name: name.to_string(), payload };
        // Clone the handlers out so a handler may emit or listen without deadlocking.
        let handlers: Vec<Handler> = self
            .0
            .listeners
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, n, _)| n.as_deref().is_none_or(|n| n == name))
            .map(|(_, _, h)| h.clone())
            .collect();
        for h in handlers {
            h(ev.clone());
        }
    }
}

/// Managed state, borrowed for a call. Derefs to `T`.
pub struct State<'r, T: Send + Sync + 'static>(Arc<T>, PhantomData<&'r T>);
impl<T: Send + Sync + 'static> Deref for State<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T: Send + Sync + 'static> Clone for State<'_, T> {
    fn clone(&self) -> Self {
        State(self.0.clone(), PhantomData)
    }
}
impl<'r, T: Send + Sync + 'static> State<'r, T> {
    pub fn inner(&self) -> &T {
        &self.0
    }
}

pub trait Manager {
    fn app_handle(&self) -> &AppHandle;

    fn manage<T: Send + Sync + 'static>(&self, value: T) -> bool {
        let mut s = self.app_handle().0.states.write().unwrap();
        if s.contains_key(&TypeId::of::<T>()) {
            return false;
        }
        s.insert(TypeId::of::<T>(), Arc::new(value));
        true
    }

    fn try_state<T: Send + Sync + 'static>(&self) -> Option<State<'_, T>> {
        let s = self.app_handle().0.states.read().unwrap();
        let any = s.get(&TypeId::of::<T>())?.clone();
        Some(State(any.downcast::<T>().ok()?, PhantomData))
    }

    fn state<T: Send + Sync + 'static>(&self) -> State<'_, T> {
        self.try_state::<T>()
            .unwrap_or_else(|| panic!("state not managed: {}", std::any::type_name::<T>()))
    }
}

impl Manager for AppHandle {
    fn app_handle(&self) -> &AppHandle {
        self
    }
}

pub trait Emitter {
    fn emit<S: serde::Serialize + Clone>(&self, event: &str, payload: S) -> Result<()>;
}

impl Emitter for AppHandle {
    fn emit<S: serde::Serialize + Clone>(&self, event: &str, payload: S) -> Result<()> {
        self.dispatch(event, serde_json::to_string(&payload)?);
        Ok(())
    }
}

pub trait Listener {
    fn listen<F: Fn(Event) + Send + Sync + 'static>(&self, event: impl Into<String>, handler: F) -> EventId;
    fn unlisten(&self, id: EventId);
}

impl Listener for AppHandle {
    fn listen<F: Fn(Event) + Send + Sync + 'static>(&self, event: impl Into<String>, handler: F) -> EventId {
        let id = self.0.next_id.fetch_add(1, Ordering::Relaxed);
        self.0.listeners.lock().unwrap().push((id, Some(event.into()), Arc::new(handler)));
        id
    }
    fn unlisten(&self, id: EventId) {
        self.0.listeners.lock().unwrap().retain(|(i, _, _)| *i != id);
    }
}

pub mod async_runtime {
    //! One multi-threaded runtime for the whole process.
    use std::future::Future;
    use std::sync::OnceLock;

    pub use tokio::task::JoinHandle;

    fn rt() -> &'static tokio::runtime::Runtime {
        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("sone-async").build().expect("tokio runtime")
        })
    }

    pub fn handle() -> tokio::runtime::Handle {
        rt().handle().clone()
    }

    pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        rt().spawn(fut)
    }

    pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        rt().spawn_blocking(f)
    }

    pub fn block_on<F: Future>(fut: F) -> F::Output {
        rt().block_on(fut)
    }
}
