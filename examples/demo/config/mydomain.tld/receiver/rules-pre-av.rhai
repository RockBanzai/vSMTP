import "prelude" as prelude;

// arbitrary putting in quarantine message if their uuid starts with `d`, `e` or `f`
const quarantine_def = rule "quarantine definition" |ctx| {
    let first_char_of_id = ctx.message_id?[0];
    if first_char_of_id in "def" {
        status::quarantine(`${first_char_of_id}`)
    } else {
        status::next()
    }
};

// transfer the ownership of the message to a clamsmtp <-> vsmtp delegation
// NOTE: should set a properties to known which rules resume on post delegation?
const delegate_to_clamav = rule "delegate to clamav" |ctx| {
    prelude::forward_to_for_all(ctx, "av");
    status::next()
};

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
        [ prelude::debug_ctx ]
    } else { [
        prelude::debug_ctx,
        rule "deny sender" |ctx| {
            if ctx.is_authenticated {
                status::next()
            } else {
                status::deny("530 5.7.0 Authentication required")
            }
        },
        rule "deny sender" |ctx| {
            if !ctx.sender.is_null && ctx.sender.domain == "spammer.tld" {
                status::deny("550 5.7.1 We don't like spammers here")
            } else {
                status::next()
            }
        },
    ] })
}

fn on_rcpt_to(ctx)   { ctx.run([ prelude::debug_ctx ]) }
fn on_pre_queue(ctx) {
    ctx.run([
        prelude::debug_ctx,
        // global::quarantine_def,
        global::delegate_to_clamav,
        prelude::debug_ctx,
    ]);
}
