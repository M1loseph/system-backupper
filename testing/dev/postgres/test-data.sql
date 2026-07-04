CREATE TABLE "TestTable" (
    "id" SERIAL PRIMARY KEY,
    "test_property" VARCHAR(255) NOT NULL
);

CREATE UNIQUE INDEX "TestIndex" 
    ON "TestTable"("test_property");

INSERT INTO "TestTable"("test_property") VALUES
    ('one'),
    ('two'),
    ('three'),
    ('four'),
    ('five');
