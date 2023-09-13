DROP DATABASE IF EXISTS example;
CREATE DATABASE example;
CREATE TABLE example.users(
    email_address   varchar(500) NOT null primary key,
    password        varchar(500) NOT null
);
INSERT INTO example.users (email_address, password) VALUES
    ('jenny.doe@example.com', 'jenny.doe'),
    ('john.doe@example.com',  'john.doe');
GRANT SELECT ON example.users TO 'tuto'
