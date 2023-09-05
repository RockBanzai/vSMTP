import "prelude" as prelude;
import "dkim-private-key" as dkim_private_key;

fn on_connect(ctx)      { ctx.run([ prelude::debug_ctx ]) }
fn on_greetings(ctx)    { ctx.run([ prelude::debug_ctx ]) }

fn on_mail_from(ctx)    { ctx.run([
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

        status::next();
    },
    prelude::debug_ctx,
]) }

fn on_rcpt_to(ctx)      { ctx.run([ prelude::debug_ctx ]) }
fn on_data(ctx)         { ctx.run([ prelude::debug_ctx ]) }
fn on_message(ctx)      { ctx.run([ prelude::debug_ctx ]) }
