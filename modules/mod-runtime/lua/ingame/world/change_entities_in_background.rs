use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use mlua::{UserData, UserDataMethods, Value};

use super::*;

/// At most this many background entity changes may wait or run at once, for
/// all mods together. A finished change counts until its result is read or its
/// job is released.
pub(crate) const MAX_ENTITY_CHANGE_JOBS: usize = 256;

/// A spawn gives the new prop's handle, a destroy gives nothing.
type Outcome = Result<Option<u32>, String>;
type Finished = Arc<Mutex<Option<(Outcome, f64)>>>;

struct Task {
    run: Box<dyn FnOnce() -> Outcome + Send>,
    done: Finished,
    _permit: Arc<Permit>,
}

struct Permit {
    in_flight: Arc<AtomicUsize>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One background thread runs the queued changes in the order they were
/// started. The existing native spawn and destroy still wait for the game to
/// confirm each change, but on this thread instead of the mods' update loop.
struct EntityChangeQueue {
    sender: Mutex<Option<Sender<Task>>>,
    in_flight: Arc<AtomicUsize>,
    limit: usize,
}

impl EntityChangeQueue {
    fn new(limit: usize) -> Self {
        Self {
            sender: Mutex::new(None),
            in_flight: Arc::new(AtomicUsize::new(0)),
            limit,
        }
    }

    fn reserve(&self) -> Option<Arc<Permit>> {
        self.in_flight
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |in_flight| {
                (in_flight < self.limit).then_some(in_flight + 1)
            })
            .ok()?;
        Some(Arc::new(Permit {
            in_flight: Arc::clone(&self.in_flight),
        }))
    }

    fn start(&self, run: Box<dyn FnOnce() -> Outcome + Send>) -> Result<EntityChangeJob, String> {
        let permit = self.reserve().ok_or_else(|| {
            format!(
                "at most {} entity changes may wait at once. Read the results of earlier changes before starting more",
                self.limit
            )
        })?;
        let done: Finished = Arc::new(Mutex::new(None));
        let task = Task {
            run,
            done: Arc::clone(&done),
            _permit: Arc::clone(&permit),
        };
        let mut sender = self
            .sender
            .lock()
            .map_err(|_| "the entity change queue failed".to_string())?;
        if sender.is_none() {
            let (queue, tasks) = mpsc::channel::<Task>();
            std::thread::Builder::new()
                .name("shroudforge-entity-changes".into())
                .spawn(move || {
                    for task in tasks {
                        let Task { run, done, _permit } = task;
                        let started = Instant::now();
                        let outcome = run();
                        // From now on only the job holds the change's place.
                        drop(_permit);
                        if let Ok(mut slot) = done.lock() {
                            *slot = Some((outcome, started.elapsed().as_secs_f64()));
                        }
                    }
                })
                .map_err(|error| format!("no thread for entity changes: {error}"))?;
            *sender = Some(queue);
        }
        if sender.as_ref().is_some_and(|queue| queue.send(task).is_ok()) {
            return Ok(EntityChangeJob {
                done,
                cached: None,
                permit: Some(permit),
            });
        }
        *sender = None;
        Err("the entity change thread stopped".to_string())
    }
}

fn shared_queue() -> &'static EntityChangeQueue {
    static QUEUE: OnceLock<EntityChangeQueue> = OnceLock::new();
    QUEUE.get_or_init(|| EntityChangeQueue::new(MAX_ENTITY_CHANGE_JOBS))
}

/// A started spawn or destroy. `result()` is nil while it waits or runs.
struct EntityChangeJob {
    done: Finished,
    cached: Option<(Outcome, f64)>,
    permit: Option<Arc<Permit>>,
}

impl UserData for EntityChangeJob {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("result", |lua, job, ()| {
            if job.cached.is_none() {
                let finished = job
                    .done
                    .lock()
                    .map_err(|_| mlua::Error::runtime("entity change: the queue failed"))?
                    .take();
                let Some(finished) = finished else {
                    return Ok(Value::Nil);
                };
                job.cached = Some(finished);
                job.permit.take();
            }
            let (outcome, seconds) = job.cached.as_ref().unwrap();
            let result = lua.create_table()?;
            result.raw_set("seconds", *seconds)?;
            match outcome {
                Ok(handle) => {
                    result.raw_set("ok", true)?;
                    if let Some(handle) = handle {
                        result.raw_set("handle", *handle)?;
                    }
                }
                Err(error) => result.raw_set("error", error.as_str())?,
            }
            Ok(Value::Table(result))
        });
    }
}

fn started_job(
    lua: &mlua::Lua,
    run: Box<dyn FnOnce() -> Outcome + Send>,
) -> mlua::Result<(LuaValue, Option<String>)> {
    match shared_queue().start(run) {
        Ok(job) => Ok((LuaValue::UserData(lua.create_userdata(job)?), None)),
        Err(reason) => Ok((LuaValue::Nil, Some(reason))),
    }
}

pub(super) fn lua_world_entity_start_spawn(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    {
        let state = lua.app_data_ref::<AppState>().unwrap();
        if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.entity.spawn") {
            return Ok((LuaValue::Nil, Some(reason)));
        }
    }
    let spawn = read_spawn_arguments(&args, "runtime.world.entity.start_spawn")?;
    started_job(
        lua,
        Box::new(move || {
            runtime_provider::world_entity_spawn(
                spawn.template_uuid,
                spawn.position,
                spawn.rotation,
                spawn.tracking,
                spawn.flags,
            )
            .map(Some)
        }),
    )
}

pub(super) fn lua_world_entity_start_destroy(
    lua: &mlua::Lua,
    args: FunctionArgs,
    r#mod: &Mod,
) -> mlua::Result<(LuaValue, Option<String>)> {
    {
        let state = lua.app_data_ref::<AppState>().unwrap();
        if let Some(reason) = runtime_denial_reason(&state, r#mod, "runtime.world.entity.destroy") {
            return Ok((LuaValue::Nil, Some(reason)));
        }
    }
    if args.len() != 1 {
        return Err(LuaError::generic(
            "runtime.world.entity.start_destroy expects entityHandle",
        ));
    }
    let handle = args.get::<u32>(0)?;
    started_job(
        lua,
        Box::new(move || runtime_provider::world_entity_destroy_handle(handle).map(|()| None)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    fn finished(job: &EntityChangeJob) -> Option<Outcome> {
        for _ in 0..500 {
            if let Some((outcome, _)) = job.done.lock().unwrap().clone() {
                return Some(outcome);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        None
    }

    #[test]
    fn changes_run_in_their_order_off_the_caller() {
        let queue = EntityChangeQueue::new(8);
        let (gate, wait) = channel::<()>();
        let order = Arc::new(Mutex::new(Vec::new()));
        let first_order = Arc::clone(&order);
        let first = queue
            .start(Box::new(move || {
                wait.recv().unwrap();
                first_order.lock().unwrap().push(1);
                Ok(Some(11))
            }))
            .unwrap();
        let second_order = Arc::clone(&order);
        let second = queue
            .start(Box::new(move || {
                second_order.lock().unwrap().push(2);
                Err("rejected".into())
            }))
            .unwrap();
        // Both started without waiting for the first change to finish.
        assert!(first.done.lock().unwrap().is_none());
        gate.send(()).unwrap();
        assert_eq!(finished(&first), Some(Ok(Some(11))));
        assert_eq!(finished(&second), Some(Err("rejected".into())));
        assert_eq!(*order.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn a_full_queue_refuses_until_a_result_is_released() {
        let queue = EntityChangeQueue::new(1);
        let job = queue.start(Box::new(|| Ok(None))).unwrap();
        assert_eq!(finished(&job), Some(Ok(None)));
        // The finished change still counts until its job lets it go.
        assert!(queue.start(Box::new(|| Ok(None))).is_err());
        drop(job);
        assert!(queue.start(Box::new(|| Ok(None))).is_ok());
    }
}
