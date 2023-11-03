import "/prelude" as prelude;

fn on_connect(ctx)   { ctx.run([ prelude::debug_ctx ]) }
fn on_helo(ctx)      { ctx.run([ prelude::debug_ctx ]) }

fn on_auth(ctx) {
    ctx.run(if ctx.server_port == 25 {
        [ prelude::debug_ctx ]
    } else { [
        prelude::debug_ctx,
        rule "shadow auth" |ctx| {
            // see https://www.gnu.org/software/gsasl/manual/gsasl.html#Mechanisms
            if ctx.sasl.mechanism == "PLAIN" || ctx.sasl.mechanism == "LOGIN" {
                prelude::run_testsaslauthd(ctx.sasl.authid, ctx.sasl.password)
            } else {
                status::next()
            }
        },
    ] })
}

fn on_mail_from(ctx) {
    ctx.run(if ctx.server_port == 25 {
        [ prelude::debug_ctx, prelude::deny_unencrypted ]
    } else { [
        prelude::debug_ctx,
        prelude::deny_unencrypted,
        prelude::deny_unauthenticated,
        rule "deny sender" |ctx| {
            if !ctx.sender.is_null() && ctx.sender.domain == "spammer.tld" {
                status::deny("550 5.7.1 We don't like spammers here")
            } else {
                status::next()
            }
        },
    ] })
}

fn on_rcpt_to(ctx)   { ctx.run([ prelude::debug_ctx ]) }
fn on_pre_queue(ctx) { ctx.run([ prelude::debug_ctx ]) }
