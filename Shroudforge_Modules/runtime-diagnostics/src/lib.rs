//! Bounded, opt-in instrumentation hosted by the loader. No scans or game writes.
use std::{collections::BTreeMap, path::{Path,PathBuf}, time::{Duration,Instant,SystemTime,UNIX_EPOCH}};
use serde_json::{Value,json};

fn seconds() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn defaults() -> Value { json!({"enabled":false,"continuous":true,"intervalMilliseconds":2000,"maximumDurationSeconds":120,"slowCallbackMilliseconds":20,"onlyChanges":true,"areas":["runtime","queue","mods"],"requestId":0}) }

#[derive(Default)]
struct Measurement { count:u64, errors:u64, total_ms:f64, maximum_ms:f64, slow:u64 }

pub struct Session {
    root:PathBuf, settings:Value, active:bool, started:Instant, next_config:Instant,
    next_sample:Instant, last_report:Value, last_error:String, reason:&'static str,
    measurements:BTreeMap<String,Measurement>, last_sample:u64,
}
impl Session {
    pub fn new(root:&Path) -> Self {
        let now=Instant::now();
        let mut session=Self {root:root.into(),settings:defaults(),active:false,started:now,next_config:now,next_sample:now,last_report:Value::Null,last_error:String::new(),reason:"disabled",measurements:BTreeMap::new(),last_sample:0};
        session.refresh();
        session.publish();
        session
    }
    pub fn enabled(&self, area:&str) -> bool {
        self.active && self.started.elapsed().as_secs() < self.limit() && self.settings["areas"].as_array().is_some_and(|areas|areas.iter().any(|item|item==area))
    }
    fn limit(&self)->u64 {self.settings["maximumDurationSeconds"].as_u64().unwrap_or(120).clamp(1,3600)}
    pub fn measure(&mut self, area:&str, id:&str, phase:&str, elapsed:Duration, failed:bool) {
        if !self.enabled(area) {return;}
        let key=format!("{area}/{id}/{phase}");
        if self.measurements.len()>=1024 && !self.measurements.contains_key(&key) {return;}
        let entry=self.measurements.entry(key).or_default();
        let ms=elapsed.as_secs_f64()*1000.0;
        entry.count+=1; entry.errors+=u64::from(failed); entry.total_ms+=ms; entry.maximum_ms=entry.maximum_ms.max(ms);
        entry.slow+=u64::from(ms>=self.settings["slowCallbackMilliseconds"].as_f64().unwrap_or(20.0));
    }
    fn refresh(&mut self) {
        if Instant::now()<self.next_config {return;}
        self.next_config=Instant::now()+Duration::from_millis(500);
        let config=match shroudforge_package::config::read_loader(&self.root) {
            Ok(value)=>value,
            Err(error)=>{if error!=self.last_error {tracing::error!(target:"shroudforge::diagnostics","Configuration rejected: {error}");self.last_error=error;}return;}
        };
        self.last_error.clear();
        let mut next=defaults();
        if let Some(values)=config["modules"]["runtimeDiagnostics"].as_object(){for (key,value) in values {next[key]=value.clone();}}
        let start=next["enabled"]==true && (self.settings["enabled"]!=true || next["requestId"]!=self.settings["requestId"]);
        if next["enabled"]!=true && self.active {self.sample();self.active=false;self.reason="stopped";self.next_sample=Instant::now();tracing::info!(target:"shroudforge::diagnostics","Diagnostic session stopped");}
        self.settings=next;
        if start {
            self.started=Instant::now();self.next_sample=self.started;self.active=true;self.reason="active";self.last_report=Value::Null;self.measurements.clear();
            tracing::info!(target:"shroudforge::diagnostics",maximum_seconds=self.limit(),"Diagnostic session started");
        }
    }
    pub fn tick(&mut self) {
        self.refresh();
        let now=Instant::now();
        if now<self.next_sample && !(self.active && self.started.elapsed().as_secs()>=self.limit()) {return;}
        self.next_sample=now+Duration::from_millis(self.settings["intervalMilliseconds"].as_u64().unwrap_or(2000).clamp(500,60000));
        if self.active {
            self.sample();
            if self.started.elapsed().as_secs()>=self.limit() || self.settings["continuous"]!=true {
                self.active=false;self.reason=if self.settings["continuous"]==true {"duration-complete"}else{"snapshot-complete"};
                tracing::info!(target:"shroudforge::diagnostics",reason=self.reason,"Diagnostic session completed");
                // Keep the same persisted source of truth; do not overwrite a newer UI request.
                let request=self.settings["requestId"].clone();
                if let Err(error)=shroudforge_package::config::update_loader(&self.root,|value| {
                    if value["modules"]["runtimeDiagnostics"].get("requestId").cloned().unwrap_or(json!(0))==request {value["modules"]["runtimeDiagnostics"]["enabled"]=false.into();}
                    Ok(())
                }) {tracing::error!(target:"shroudforge::diagnostics","Could not persist completed session: {error}");}
            }
        }
        self.publish();
    }
    fn sample(&mut self) {
        let mut report=json!({});
        if self.enabled("runtime") || self.enabled("queue") {
            let observed=native_snapshot();
            let mut native=json!({"layoutReady":observed["layoutReady"],"layoutEpoch":observed["layoutEpoch"],"dispatcher":observed["dispatcher"],"error":observed["error"]});
            if let Some(age)=native.pointer("/dispatcher/lastDrainAgeMs").and_then(Value::as_u64) {
                native["dispatcher"]["drainObservedWithinFiveSeconds"]=json!(age<=5000);
            }
            if !self.enabled("queue") {native.as_object_mut().map(|object|object.remove("dispatcher"));}
            if !self.enabled("runtime") {native=json!({"dispatcher":native["dispatcher"],"error":native["error"]});}
            report["native"]=native;
        }
        for (key,entry) in &self.measurements {
            report["measurements"][key]=json!({"count":entry.count,"errors":entry.errors,"totalMs":entry.total_ms,"maximumMs":entry.maximum_ms,"slowCalls":entry.slow});
        }
        // Compare state, not ever-changing ages/counters, to suppress identical health reports.
        let mut signature=report.clone();
        if let Some(dispatcher)=signature.pointer_mut("/native/dispatcher").and_then(Value::as_object_mut) {
            for key in ["lastDrainAgeMs","drainCount","completed","lastManagerObservationAgeMs"] {dispatcher.remove(key);}
        }
        if self.settings["onlyChanges"]!=true || signature!=self.last_report {
            let slow=self.measurements.values().any(|entry|entry.slow>0 || entry.errors>0) || report.pointer("/native/error").is_some_and(|error|!error.is_null());
            if slow {tracing::warn!(target:"shroudforge::diagnostics",report=%report,"Diagnostic measurements");}
            else {tracing::info!(target:"shroudforge::diagnostics",report=%report,"Diagnostic measurements");}
            self.last_report=signature;
        }
        self.last_sample=seconds();
    }
    fn publish(&self) {
        let logging=shroudforge_package::config::read_loader(&self.root).ok();
        let value=json!({"schemaVersion":1,"pid":std::process::id(),"updatedAt":seconds(),"active":self.active,
            "reason":self.reason,"remainingSeconds":if self.active {self.limit().saturating_sub(self.started.elapsed().as_secs())}else{0},
            "lastSampleAt":self.last_sample,"loggingEnabled":logging.as_ref().is_some_and(|v|v["logging"]["enabled"]==true),
            "minimumLevel":logging.as_ref().and_then(|v|v["logging"]["minimumLevel"].as_str()).unwrap_or("INFO")});
        if let Err(error)=shroudforge_package::config::write_document(&self.root,"diagnostics-status",&value) {
            tracing::error!(target:"shroudforge::diagnostics","Status publication failed: {error}");
        }
    }
}
impl Drop for Session {fn drop(&mut self){if self.active {self.sample();}self.active=false;self.reason="process-stopped";self.publish();}}

pub fn request(root:&Path,action:&str)->Result<(),String> {
    if !["start","stop","snapshot"].contains(&action){return Err("invalid diagnostic action".into());}
    shroudforge_package::config::update_loader(root,|value| {
        let settings=&mut value["modules"]["runtimeDiagnostics"];
        settings["enabled"]=json!(action!="stop");
        if action!="stop" {
            settings["continuous"]=json!(action=="start");
            settings["requestId"]=json!(settings["requestId"].as_u64().unwrap_or(0).saturating_add(1));
        }
        Ok(())
    })
}

pub fn status(root:&Path)->Value {
    let result=shroudforge_package::config::read_document(root,"diagnostics-status");
    match result {
        Ok(mut value)=>{let fresh=value["updatedAt"].as_u64().is_some_and(|time|time<=seconds() && seconds()-time<=65);value["fresh"]=fresh.into();if !fresh{value["active"]=false.into();value["reason"]="process-not-reporting".into();}value},
        Err(error)=>json!({"active":false,"fresh":false,"reason":"no-runtime-report","detail":error}),
    }
}

#[cfg(windows)]
fn native_snapshot()->Value {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW,GetProcAddress};
    let name:Vec<u16>="kfc-runtime.dll\0".encode_utf16().collect();
    unsafe {
        let module=GetModuleHandleW(name.as_ptr());
        if module.is_null(){return json!({"error":"runtime-provider-not-loaded"});}
        let Some(symbol)=GetProcAddress(module,c"KfcRuntimeDiagnostics".as_ptr().cast()) else {return json!({"error":"runtime-provider-missing-diagnostics-export"});};
        let function:unsafe extern "C" fn(*mut u8,usize)=std::mem::transmute(symbol);
        let mut buffer=vec![0u8;16384];function(buffer.as_mut_ptr(),buffer.len());
        let length=buffer.iter().position(|byte|*byte==0).unwrap_or(buffer.len());
        serde_json::from_slice(&buffer[..length]).unwrap_or_else(|error|json!({"error":format!("invalid-provider-report: {error}")}))
    }
}
#[cfg(not(windows))]
fn native_snapshot()->Value {json!({"error":"native-provider-requires-windows"})}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_means_no_measurements_and_snapshot_finishes() {
        let root=tempfile::tempdir().unwrap();
        let mut session=Session::new(root.path());
        session.measure("mods","example","on_update",Duration::from_millis(30),false);
        assert!(session.measurements.is_empty());
        request(root.path(),"snapshot").unwrap();
        session.next_config=Instant::now();session.tick();
        assert!(!session.active);assert_eq!(session.reason,"snapshot-complete");
        assert!(session.last_sample>0);
        assert_eq!(shroudforge_package::config::read_loader(root.path()).unwrap()["modules"]["runtimeDiagnostics"]["enabled"],false);
        assert_eq!(status(root.path())["reason"],"snapshot-complete");
    }
    #[test]
    fn deadline_is_independent_of_sample_interval_and_restart_works() {
        let root=tempfile::tempdir().unwrap();
        shroudforge_package::config::update_loader(root.path(),|value|{
            value["modules"]["runtimeDiagnostics"]["maximumDurationSeconds"]=json!(1);
            value["modules"]["runtimeDiagnostics"]["intervalMilliseconds"]=json!(60000);Ok(())
        }).unwrap();
        request(root.path(),"start").unwrap();
        let mut session=Session::new(root.path());
        session.measure("mods","example","on_update",Duration::from_millis(30),true);
        assert_eq!(session.measurements["mods/example/on_update"].slow,1);
        session.next_sample=Instant::now()+Duration::from_secs(60);
        session.started=Instant::now()-Duration::from_secs(2);
        session.tick();assert!(!session.active);assert_eq!(session.reason,"duration-complete");
        request(root.path(),"start").unwrap();session.next_config=Instant::now();session.tick();
        assert!(session.active);assert!(session.measurements.is_empty());
        request(root.path(),"stop").unwrap();session.next_config=Instant::now();session.tick();
        assert!(!session.active);assert_eq!(status(root.path())["reason"],"stopped");
    }
    #[test]
    fn invalid_settings_are_rejected() {
        let root=tempfile::tempdir().unwrap();
        assert!(shroudforge_package::config::update_loader(root.path(),|value|{
            value["modules"]["runtimeDiagnostics"]["maximumDurationSeconds"]=json!(0);Ok(())
        }).is_err());
        assert!(request(root.path(),"fake-action").is_err());
    }
}
