import "/etc/vsmtp/plugins/libvsmtp_plugin_mysql" as mysql;

const users_db = mysql::connect(#{
    url: "mysql://users:3306/?user=tuto&password=tuto",
    connections: 1,
    timeout: "3s",
});

fn on_auth(ctx) {
    if ctx.sasl.mechanism == "PLAIN" || ctx.sasl.mechanism == "LOGIN" {
        let user = global::users_db.query(
            `SELECT password FROM example.users WHERE email_address = '${ctx.sasl.authid}';`
        );

        if user.shift()?.password == `'${ctx.sasl.password}'` {
            return status::accept()
        }
    }
    status::next()
}

fn on_mail_from(ctx) {
    if ctx.server_port() == 587 && !ctx.is_authenticated {
        status::deny("530 5.7.0 Authentication required")
    } else {
        status::next()
    }
}
