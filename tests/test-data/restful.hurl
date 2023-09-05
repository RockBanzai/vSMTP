POST http://192.168.1.32:8080/relay
Content-Type: application/json
# FIXME: the `connect` property should be set by the MTA
{
    "connect": {
        "connect_timestamp": "+002023-05-26T12:08:48.577569980Z",
        "connect_uuid": "2817f035-cef8-4db6-93f0-1c757b68c50f",
        "client_addr": "127.0.0.1:50000",
        "server_addr": "192.168.1.32:8080",
        "server_name": "api.mydomain.tld"
    },
    "helo": {
        "client_name": "localhost",
        "using_deprecated": false
    },
    "mail_from": {
        "reverse_path": "foo@mydomain.tld",
        "mail_timestamp": "+002023-05-26T12:08:48.577569980Z",
        "message_uuid": "2817f035-cef8-4db6-93f0-1c757b68c50f"
    },
    "rcpt_to": {
        "recipient": {
            "forward.av": [
                {
                    "forward_path": "foo@mydomain.tld",
                    "notify_on": {
                        "Some": {
                            "success": true,
                            "failure": true,
                            "delay": true
                        }
                    }
                }
            ]
        }
    },
    "message": {
        "raw": {
            "headers": [
                "From: <foo@mydomain.tld>\r\n",
                "To: <foo@mydomain.tld>\r\n",
                "Date: Fri, 26 May 2023 14:08:48 +0200 (CEST)\r\n"
            ],
            "body": "La de da de da 1.\r\nLa de da de da 2.\r\nLa de da de da 3.\r\nLa de da de da 4.\r\n"
        }
    }
}
HTTP 200
