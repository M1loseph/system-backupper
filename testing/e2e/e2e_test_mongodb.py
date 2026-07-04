from pymongo import MongoClient
import requests

DATABASE_NAME = "testdb"
COLLECTION_NAME = "test_collection"

def __insert_test_source_data(source_client):
    source_db = source_client[DATABASE_NAME]
    source_collection = source_db[COLLECTION_NAME]
    source_collection.delete_many({})
    source_collection.insert_many([
        {
           "name": "first",
           "value": 42
        },
        {
           "name": "second",
           "value": 43
        },
    ])

def __insert_test_target_data(target_client):
    target_db = target_client[DATABASE_NAME]
    target_collection = target_db[COLLECTION_NAME]
    target_collection.delete_many({})
    target_collection.insert_one(
        {
           "name": "third",
           "value": 44
        },
    )

def test_health_endpoint_returns_true():
    response = requests.post("http://localhost:2000/api/v1/targets/mongodbSource/health")
    response.raise_for_status()
    assert response.json() == {"is_healthy": True}

    response = requests.post("http://localhost:2000/api/v1/targets/mongodbTarget/health")
    response.raise_for_status()
    assert response.json() == {"is_healthy": True}

def test_health_endpoint_returns_false_for_wrong_target():
    response = requests.post("http://localhost:2000/api/v1/targets/mongodbWithTypo/health")
    response.raise_for_status()
    assert response.json() == {"is_healthy": False}

def test_create_backup_and_restore_it_to_collection_that_contains_documents():
    # given
    source_client = MongoClient("mongodb://username:password@localhost:27017/")
    __insert_test_source_data(source_client)

    target_client = MongoClient("mongodb://username:password@localhost:27016/")
    __insert_test_target_data(target_client)

    # when
    response = requests.post("http://localhost:2000/api/v1/backups/mongodbSource")
    response.raise_for_status()
    response_body = response.json()
    backup_id = response_body["backup_id"]

    response = requests.post(f"http://localhost:2000/api/v1/targets/mongodbTarget/backups/{backup_id}")
    response.raise_for_status()

    # then
    target_collection = target_client[DATABASE_NAME][COLLECTION_NAME]
    restored_documents = list(target_collection.find({}, {"_id": 0}).sort("value", 1))
    assert restored_documents == [
        {
            "name": "first",
            "value": 42
        },
        {
            "name": "second",
            "value": 43
        },
        {
            "name": "third",
            "value": 44
        }
    ]


def test_create_backup_and_restore_it_to_collection_that_contains_documents_with_drop_flag():
    # given
    source_client = MongoClient("mongodb://username:password@localhost:27017/")
    __insert_test_source_data(source_client)

    target_client = MongoClient("mongodb://username:password@localhost:27016/")
    __insert_test_target_data(target_client)

    # when
    response = requests.post("http://localhost:2000/api/v1/backups/mongodbSource")
    response.raise_for_status()
    response_body = response.json()
    backup_id = response_body["backup_id"]

    response = requests.post(f"http://localhost:2000/api/v1/targets/mongodbTarget/backups/{backup_id}?drop=true")
    response.raise_for_status()

    # then
    target_collection = target_client[DATABASE_NAME][COLLECTION_NAME]
    restored_documents = list(target_collection.find({}, {"_id": 0}).sort("value", 1))
    assert restored_documents == [
        {
            "name": "first",
            "value": 42
        },
        {
            "name": "second",
            "value": 43
        }
    ]
