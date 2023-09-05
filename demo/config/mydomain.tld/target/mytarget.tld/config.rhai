fn on_config(config) {
    #{
        "route": "mytarget.tld",
        "dns": #{
            "mode": "lookup_mx",
            "resolver": #{
                "config": #{
                    "name_servers": [
                        #{
                            "socket_addr": "172.23.0.1:53"
                        }
                    ]
                },
                "option": #{
                    "attempts": 1
                }
            }
        },
        "smtp": #{
            "bind_address": [
                "0.0.0.0:10000",
                "0.0.0.0:20000",
                "0.0.0.0:30000"
            ]
        },
        "amqp_throttle": "1/1ms",
        "target_pool": #{
            "max_size": 1,
            "min_idle": 1,
            "idle_timeout": "10s",
            "max_lifetime": "1m",
            "connection_timeout": "20s"
        },
        "quota": #{
            // "rcpt_per_message": 100,
            "message_per_connection": 1
        },
        "tokio_runtime": #{
            "worker_threads": 8,
            "max_blocking_threads": 512,
            "thread_stack_size": 2097152,
            "thread_keep_alive": "10s",
            "global_queue_interval": 61,
            "event_interval": 61,
            "max_io_events_per_tick": 1024
        },
        "broker": #{
            "uri": "amqps://sink-mytarget:password@rabbitmq.mydomain.tld:5671/sink-mytarget",
            "certificate_chain": "/etc/ssl/certs/ca-certificates.crt"
        },
        "logs": #{
            "levels": #{
                "receiver": "warn",
                "delivery": "warn"
            }
        }
    }
}
