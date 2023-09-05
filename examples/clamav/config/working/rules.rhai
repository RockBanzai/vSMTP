import "clamav-service" as svc;

fn on_post_queue(ctx) {
    ctx.run([
        rule "run antivirus" |ctx| {
            if svc::antivirus.scan(ctx) {
                status::quarantine("virus")
            } else {
                status::success()
            }
        }
    ]);
}