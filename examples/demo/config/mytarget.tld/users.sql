DROP DATABASE IF EXISTS mytarget;
CREATE DATABASE mytarget;
CREATE TABLE mytarget.users(
    email_address   varchar(500) NOT null primary key,
    password        varchar(500) NOT null
);
INSERT INTO mytarget.users (email_address, password)
    VALUES ('jenny.doe@mytarget.tld', 'jenny.doe');
GRANT SELECT ON mytarget.users TO 'vsmtp-dev'
