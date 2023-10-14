import "prelude" as prelude;
import "/etc/vsmtp/plugins/libvsmtp_plugin_mysql" as mysql;

const MY_DOMAINS = ["mytarget.tld"];

const users_db = mysql::connect(#{
    url: "mysql://mytarget-users-db:3306/?user=vsmtp-dev&password=vsmtp-dev",
    connections: 4,
    timeout: "3s",
});

const dns_resolver = dns::resolver(#{
    "config": #{
        "name_servers": [
            #{
                "socket_addr": "172.23.0.1:53"
            }
        ]
    },
    "option": #{
        "attempts": 1,
    }
});

fn get_spf_policy(ctx) {
    // for testing purposes
    if ctx.helo == "spf.no.deny" || rand_bool() {
        "relaxed"
    } else {
        "strict"
    }
}

const rules = #{
    "25": #{
        connect: |ctx| {
            // 70% of the time, we'll do a reverse DNS lookup
            ctx.run(if (rand() % 100) >= 30 {
                [ prelude::iprev_rule ]
            } else {
                [ prelude::debug_ctx ]
            })
        },
        helo:       |ctx| { ctx.run([ prelude::spf_rule(#{
            identity: "helo",
            policy: get_spf_policy(ctx)
        }) ]) },
        mail_from:  |ctx| { ctx.run([
            prelude::deny_unencrypted,
            prelude::spf_rule(#{
                identity: "mail_from",
                policy: get_spf_policy(ctx)
            })
        ]) },
        rcpt_to:    |ctx| { ctx.run([
            rule "check if recipient exists" |ctx| {
                // FIXME: only check the last rcpt?
                for i in ctx.recipients {
                    if global::MY_DOMAINS.contains(i.domain) {
                        let user = global::users_db.query(
                            `SELECT * FROM mytarget.users WHERE email_address = '${i.address}';`
                        );
                        if user.is_empty() {
                            return status::deny("550 5.1.1 Bad destination mailbox address");
                        }
                    }
                }
                status::next()
            },
        ]) },
        pre_queue:  |ctx| { ctx.run([
            prelude::dkim_rule,
            prelude::dmarc_rule,
            rule "add auth result" |ctx| {
                auth::add_header(ctx, #{ auth_serv_id: "mytarget.tld" });
                status::next()
            },
            prelude::debug_ctx,
            rule "set routing path" |ctx| {
                for i in ctx.recipients {
                    if global::MY_DOMAINS.contains(i.domain) {
                        // set a routing maildir for my domain's recipients
                        ctx.set_routing_path(i, "maildir");
                    } else {
                        // nothing to do here, default value is `basic`
                    }
                }
                status::next()
            },
        ]) }
    },
    "587": #{
        connect:    |ctx| { ctx.run([ prelude::debug_ctx ]) },
        helo:       |ctx| { ctx.run([ prelude::debug_ctx ]) },
        auth:       |ctx| { ctx.run([
            rule "shadow auth" |ctx| {
                // see https://www.gnu.org/software/gsasl/manual/gsasl.html#Mechanisms
                if ctx.sasl.mechanism == "PLAIN" || ctx.sasl.mechanism == "LOGIN" {
                    let user = global::users_db.query(
                        `SELECT password FROM mytarget.users WHERE email_address = '${ctx.sasl.authid}';`
                    );

                    // FIXME: for some reason, quote are added somewhere
                    if user.shift()?.password == `'${ctx.sasl.password}'` {
                        status::accept()
                    } else {
                        status::next()
                    }
                } else {
                    status::next()
                }
            },
        ]) },
        mail_from:  |ctx| { ctx.run([
            prelude::debug_ctx,
            prelude::deny_unencrypted,
            prelude::deny_unauthenticated,
        ]) },
        rcpt_to:    |ctx| { ctx.run([ prelude::debug_ctx ]) },
        pre_queue:  |ctx| { ctx.run([ prelude::debug_ctx ]) }
    }
};

fn on_connect(ctx) {
    global::rules[ctx.server_port.to_string()].connect(ctx)
}

fn on_helo(ctx) {
    global::rules[ctx.server_port.to_string()].helo(ctx)
}

fn on_auth(ctx) {
    global::rules[ctx.server_port.to_string()].auth(ctx)
}

fn on_mail_from(ctx) {
    global::rules[ctx.server_port.to_string()].mail_from(ctx)
}

fn on_rcpt_to(ctx) {
    global::rules[ctx.server_port.to_string()].rcpt_to(ctx)
}

fn on_pre_queue(ctx) {
    global::rules[ctx.server_port.to_string()].pre_queue(ctx)
}
