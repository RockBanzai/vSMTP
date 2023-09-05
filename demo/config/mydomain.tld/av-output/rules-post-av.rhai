import "prelude" as prelude;

fn on_connect(ctx)   { ctx.run([ prelude::debug_ctx ]) }
fn on_helo(ctx)      { ctx.run([ prelude::debug_ctx ]) }
fn on_mail_from(ctx) { ctx.run([ prelude::debug_ctx ]) }
fn on_rcpt_to(ctx)   { ctx.run([ prelude::debug_ctx ]) }
fn on_pre_queue(ctx) {
    ctx.run([
        prelude::debug_ctx,
        rule "check antivirus status" |ctx| {
            if ctx.has_header("X-Virus-Infected") {
                status::quarantine("virus")
            } else {
                status::next()
            }
        },
        rule "set routing path" |ctx| {
            const MY_DOMAINS = ["mydomain.tld"];

            for i in ctx.recipients {
                if MY_DOMAINS.contains(i.domain) {
                    // set a routing maildir for my domain's recipients
                    ctx.set_routing_path(i, "maildir");
                } else {
                    // nothing to do here, default value is `basic`
                }
            }
            status::next()
        },
        prelude::debug_ctx,
    ])
}
