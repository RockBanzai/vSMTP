# vsmtp-plugin-sqlite

> Plugin to connect to a sqlite database

## Build

```rust
$ cargo build
```

## Usage

> First, you will need a greylist.db file with that configuration inside

```sql
CREATE TABLE IF NOT EXISTS greylist_sender(
    address varchar(500) NOT null primary key,
    user varchar(500) NOT null,
    domain varchar(500) NOT null
)
```

> In the services/db.rhai, you can have this type of configuration to get your database as a service

```rust
import "plugins/libvsmtp_plugin_sqlite" as sqlite;

// A service used to connect to and query our greylist database.
export const greylist = sqlite::connect(#{
    path: "greylist.db",
    connections: 4,
    timeout: "3s"
});

```

> In the filter.rhai file, you can have this type of configuration for a greylist service

```rust
import "services/db" as db;

#{
    mail: [
        rule "log transaction" || {
            let sender = ctx::mail_from();

            // if the sender is not recognized in our database,
            // we deny the transaction and write the sender into
            // the database.
            //
            // In this example, we use a sqlite table called "sender" in a "greylist" database.
            if db::greylist.query(`SELECT * FROM greylist.sender WHERE address = '${sender}';`) == [] {
                log("info", `New client discovered: ${sender}`);
                db::greylist.query(`
                    INSERT INTO greylist.sender (user, domain, address)
                    values ("${sender.local_part}", "${sender.domain}", "${sender}");
                `);

                state::deny(code::c451_7_1())
            } else {
                log("info", `Known client connected: ${sender}`);
                // the user is known by the server, the transaction
                // can proceed.
                state::accept()
            }
        },
    ],

    delivery: [
        rule "setup delivery" || state::quarantine("hold")
    ]
}

```
