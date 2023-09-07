import "prelude" as prelude;
import "dkim-private-key" as dkim_private_key;

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
        rule "double dkim signature" |ctx| {
            // using `dkim::add_signature`
            dkim::add_signature(ctx.mail, #{
                sdid:           "mydomain.tld",
                selector:       "rsa-dkim",
                private_key:    dkim_private_key::rsa,
                headers_field:  ["From", "To", "Date", "From", "Subject", "Message-Id"]
            });

            // or using `dkim::create_sign` + `msg::prepend_header`
            let ed_signature = dkim::create_sign(ctx.mail, #{
                sdid:           "mydomain.tld",
                selector:       "ed-dkim",
                private_key:    dkim_private_key::ed,
                headers_field:  ["From", "To", "Date", "From", "Subject", "Message-Id"]
            });
            ctx.prepend_header("DKIM-Signature", ed_signature);

            status::next()
        },
        prelude::debug_ctx,
    ])
}
