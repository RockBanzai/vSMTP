// NOTE: this file can contain helper function, but not directly rules
// there is a function pointer bugs between modules :/
// see https://rhai.rs/book/language/fn-ptr.html?highlight=function%20pointer#warning--global-namespace-only

fn forward_to(ctx, i, service) {
    ctx.set_routing_path(i, `forward.${service}`);
}

fn forward_to_for_all(ctx, service) {
    for i in ctx.recipients {
        forward_to(ctx, i, service);
    }
}

export const debug_ctx = rule "debug ctx" |ctx| {
    debug(ctx);
    status::next()
};

//
// TLS
//

export const deny_unencrypted = rule "deny unencrypted" |ctx| {
    if ctx.is_secured() {
        status::next()
    } else {
        status::deny(`523 5.7.10 unencrypted connections are not allowed`)
    }
};

//
// Authentication
//

// SASL

fn run_testsaslauthd(authid, password) {
    debug(authid);
    debug(password);
    let saslauthd = process::run(#{
        args: [
            "testsaslauthd",
            "-u", authid,
            "-p", password,
        ]
    });
    // print(saslauthd.stderr);
    // print(saslauthd.stdout);

    // checking against the stdout, could also use the exit code
    if saslauthd.stdout == "0: OK \"Success.\"\n" {
        status::accept()
    } else {
        status::next()
    }
}

export const deny_unauthenticated = rule "deny unauthenticated" |ctx| {
    debug(ctx);
    if ctx.is_authenticated {
        status::next()
    } else {
        status::deny("530 5.7.0 Authentication required")
    }
};

// FIXME: those rules use global variable, it should be passed as a parameter

// IpRev

export const iprev_rule = rule "iprev rule" |ctx| {
    let iprev_res = iprev::check(#{
        ip: ctx.client_ip,
        dns_resolver: global::dns_resolver
    });

    if iprev_res.value != "pass" {
        // NOTE: is the reply code different depending on the value (fail, temperror, permerror) ?
        status::deny(`550 5.7.25 the sending address ${ctx.client_ip} must have a reverse DNS record`)
    } else {
        ctx.store(iprev_res);
        status::next()
    }
};

// SPF

export const spf_rule_helo_strict = rule "spf rule helo strict" |ctx| {
    let helo_identity = spf::check_host(#{
        ip: ctx.client_ip,
        helo: ctx.helo,
        dns_resolver: global::dns_resolver
    });
    if helo_identity != "pass" {
        status::deny(`550 5.7.23 ${ctx.helo} is not allowed to send mail from ${ctx.client_ip}`)
    } else {
        ctx.store("helo", helo_identity);
        status::next()
    }
};

export const spf_rule_mail_from_strict = rule "spf rule mail from strict" |ctx| {
    let mail_from_identity = spf::check_host(#{
        ip: ctx.client_ip,
        helo: ctx.helo,
        mail_from: ctx.sender,
        dns_resolver: global::dns_resolver
    });
    if mail_from_identity != "pass" {
        status::deny(`550 5.7.23 ${ctx.sender} is not allowed to send mail from ${ctx.client_ip}`)
    } else {
        ctx.store("mail_from", mail_from_identity);
        status::next()
    }
};

export const spf_rule_helo_relaxed = rule "spf rule helo relaxed" |ctx| {
    let helo_identity = spf::check_host(#{
        ip: ctx.client_ip,
        helo: ctx.helo,
        dns_resolver: global::dns_resolver
    });
    ctx.store("helo", helo_identity);
    status::next()
};

export const spf_rule_mail_from_relaxed = rule "spf rule mail from relaxed" |ctx| {
    let mail_from_identity = spf::check_host(#{
        ip: ctx.client_ip,
        helo: ctx.helo,
        mail_from: ctx.sender,
        dns_resolver: global::dns_resolver
    });
    ctx.store("mail_from", mail_from_identity);
    status::next()
};

fn spf_rule(params) {
    // FIXME: should not call the module from itself
    switch [params?.policy, params?.identity] {
        ["strict",  "helo"]         => prelude::spf_rule_helo_strict,
        ["relaxed", "helo"]         => prelude::spf_rule_helo_relaxed,
        ["strict",  "mail_from"]    => prelude::spf_rule_mail_from_strict,
        ["relaxed", "mail_from"]    => prelude::spf_rule_mail_from_relaxed,
        _ => throw "unknown identity/policy"
    };
}

// DKIM

// TODO: add verification of the output, and return status::deny if it fails?
// see https://datatracker.ietf.org/doc/html/rfc7372#section-3.1
export const dkim_rule = rule "dkim verify" |ctx| {
    let dkim_result = dkim::verify(ctx.mail, #{ dns_resolver: global::dns_resolver });
    ctx.store(dkim_result);
    status::next()
};

// DMARC

export const dmarc_rule = rule "dmarc check" |ctx| {
    let dmarc_result = dmarc::check(ctx, #{ dns_resolver: global::dns_resolver });
    ctx.store(dmarc_result);

    if dmarc_result.value == "pass" {
        status::next()
    } else {
        switch dmarc_result.policy {
            "none" => {
                // TODO: let the user decide what to do?
                log("app", "warn", "the message failed the DMARC check but DMARC policy is none, ignoring");
                status::next()
            }
            "quarantine" => status::quarantine("dmarc"),
            "reject" => status::deny(`550 5.7.25 DMARC policy violation`),
            _ => throw "unknown DMARC policy"
        }
    }
};
