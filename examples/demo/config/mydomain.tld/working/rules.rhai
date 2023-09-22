import "/etc/vsmtp/plugins/libvsmtp_plugin_clamav" as clamav;
import "dkim-private-key" as dkim_private_key;

const bridge = clamav::connect(#{
    url: "tcp://clamav:3310",
});

fn on_post_queue(ctx) {
    ctx.run([
        rule "check antivirus status" |ctx| {
            if global::bridge.scan(ctx) {
                status::quarantine("virus")
            } else {
                status::next()
            }
        },
        rule "set routing path" |ctx| {
            for i in ctx.recipients {
                if i.domain is "mydomain.tld" {
                    // set a routing maildir for my domain's recipients
                    ctx.set_routing_path(i, "maildir");
                } else {
                    // nothing to do here, default value is `basic`, equivalent to:
                    // ctx.set_routing_path(i, "basic");
                }
            }

            status::next()
        },
        rule "double dkim signature" |ctx| {
            // using `dkim::sign`
            dkim::sign(ctx.mail, #{
                sdid:           "mydomain.tld",
                selector:       "rsa-dkim",
                private_key:    dkim_private_key::rsa,
                headers_field:  ["From", "To", "Date", "From", "Subject", "Message-Id"]
            });

            // or using `dkim::create_signature` + `msg::prepend_header`
            let ed_signature = dkim::create_signature(ctx.mail, #{
                sdid:           "mydomain.tld",
                selector:       "ed-dkim",
                private_key:    dkim_private_key::ed,
                headers_field:  ["From", "To", "Date", "From", "Subject", "Message-Id"]
            });
            ctx.prepend_header("DKIM-Signature", ed_signature);

            status::next()
        },
    ])
}
