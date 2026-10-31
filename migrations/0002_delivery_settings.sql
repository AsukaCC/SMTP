CREATE TABLE delivery_settings (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    enabled BOOLEAN NOT NULL
);
