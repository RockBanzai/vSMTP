import "domain-enabled-resolver" as domains;
import "prelude" as prel;

const defaults = #{

    connect: [
        action "log connection" |ctx| print(`client: ${ctx.client_ip()}:${ctx.client_port()}`),
        rule   "do stuff"      |ctx| status::ok(),
    ],

    mail_from: [
        rule "close transaction for unknown sender" |ctx| status::fail("default used"),
    ],

    rcpt_to: [
        rule "reject relay" |ctx| status::fail("550 5.7.1 Relaying not allowed"),
    ],

};

fn on_connect(ctx) {
    // Simply run the defaults.
    ctx.run(global::defaults.connect)
}

fn on_helo(ctx) {
    // You can also execute inline rules.
    ctx.run([
        action "log helo" |ctx| print(`helo: ${ctx.helo()}`),
        rule   "do stuff" |ctx| status::ok("helo accepted"),
    ])
}

fn on_mail_from(ctx) {
    let domain = if !ctx.sender.is_null {
        ctx.sender.domain
    } else {
        ()
    };

    // if the sender's domain is not handled nor enabled, we reject it.
    ctx.run(domains::rules, domain) ?? ctx.run(global::defaults.mail_from)
}

fn on_rcpt_to(ctx) {
    let flow = ctx.flow(domains::rules);

    if flow != () {
        print(`flow: ${flow.type}, domain: ${flow.domain}`);
    }

    // if the recipient's domain is not handled nor enabled, we reject it.
    // Also, we get the flow of the email to check wich rules to apply.
    ctx.run(domains::rules, flow) ?? ctx.run(global::defaults.rcpt_to)
}
