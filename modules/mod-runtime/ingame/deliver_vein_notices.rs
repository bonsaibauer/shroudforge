use super::{
    IngameRuntime,
    vein_notice_pump::{Batch, Event},
};
impl IngameRuntime {
    pub(super) fn deliver_vein_notices(&mut self, client: bool) {
        let root = &self.root;
        self.vein_notices.tick(
            client,
            || {
                shroudforge_runtime_bridge::take_vein_notices()
                    .ok()
                    .map(|batch| Batch {
                        toggle: batch.toggle,
                        radius: batch.radius,
                    })
            },
            |event| {
                let (id, message) = match event {
                    Event::Toggle(value) => (
                        "vein-toggle",
                        format!("Vein Mining {}", if value { "On" } else { "Off" }),
                    ),
                    Event::Radius(value) => ("vein-radius", format!("Vein Mining radius x{value}")),
                };
                crate::notice_delivery::publish(root, "shroudforge", id, "Vein Mining", &message, 0)
                    .is_ok()
            },
        );
    }
}
