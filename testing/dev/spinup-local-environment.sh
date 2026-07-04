#!/bin/bash
set -e

docker compose down
docker compose up -d

TEST_DATA=$(ls mongo/*json)

for FILE_PATH in ${TEST_DATA[@]}; do
    FILE=${FILE_PATH##*/}
    COLLECTION_NAME=${FILE%.json}
    echo "Importing ${COLLECTION_NAME} from file ${FILE} into mongo..."
    mongoimport --uri mongodb://username:password@localhost:27017/testdb?authSource=admin --collection "${COLLECTION_NAME}" --type json --jsonArray --file "${FILE_PATH}"
done

echo "Starting creating an index in mongodb..."

MONGO_CONTAINER_ID=$(docker ps --filter name=system-backuper-mongo -q)

docker exec ${MONGO_CONTAINER_ID} mongosh -u username -p password \
    --eval "use testdb" \
    --eval 'db.testCollectionWithIndex.createIndex({"test": 1}, {unique: true, name: "TestIndex"})'

POSTGRES_CONTAINER_ID=$(docker ps --filter="name=system-backuper-postgres" -q)

PGPASSWORD=password psql -U username -h localhost -d testdb -f postgres/test-data.sql
