import psycopg
import requests

TABLE_NAME = "test_table"

def __source_connection():
    conn = psycopg.connect("postgresql://username:password@localhost:5432/testdb")
    conn.autocommit = True
    with conn.cursor() as cur:
        cur.execute(f"DROP TABLE IF EXISTS {TABLE_NAME};")
    return conn


def __target_connection():
    conn = psycopg.connect("postgresql://username:password@localhost:5431/testdb")
    conn.autocommit = True
    with conn.cursor() as cur:
        cur.execute(f"DROP TABLE IF EXISTS {TABLE_NAME};")
    return conn

def test_health_endpoint_returns_true_for_postgres_source():
    response = requests.post("http://localhost:2000/api/v1/targets/postgresSource/health")
    response.raise_for_status()
    assert response.json() == {"is_healthy": True}

    response = requests.post("http://localhost:2000/api/v1/targets/postgresTarget/health")
    response.raise_for_status()
    assert response.json() == {"is_healthy": True}

def test_health_endpoint_returns_false_for_wrong_target():
    response = requests.post("http://localhost:2000/api/v1/targets/postgresWithTypo/health")
    response.raise_for_status()
    assert response.json() == {"is_healthy": False}

def test_create_backup_and_restore_it_to_an_empty_database():
    source_conn = __source_connection()
    target_conn = __target_connection()
    # given
    with source_conn.cursor() as source_cur:
        source_cur.execute(f"""
            CREATE TABLE {TABLE_NAME} (
                id INT PRIMARY KEY,
                name VARCHAR(50),
                value INT
            );
            INSERT INTO {TABLE_NAME} (id, name, value) VALUES
                (1, 'first', 42),
                (2, 'second', 43);
        """)

    # when
    response = requests.post("http://localhost:2000/api/v1/backups/postgresSource")
    response.raise_for_status()
    response_body = response.json()
    backup_id = response_body["backup_id"]

    response = requests.post(f"http://localhost:2000/api/v1/targets/postgresTarget/backups/{backup_id}")
    response.raise_for_status()

    # then
    with target_conn.cursor() as target_cur:
        target_cur.execute(f"SELECT id, name, value FROM {TABLE_NAME} ORDER BY value;")
        restored_rows = target_cur.fetchall()
        assert restored_rows == [
            (1, 'first', 42),
            (2, 'second', 43),
        ]

def test_create_backup_and_restore_it_to_database_with_existing_table():
    source_conn = __source_connection()
    target_conn = __target_connection()
    # given
    with source_conn.cursor() as source_cur:
        source_cur.execute(f"""
            CREATE TABLE {TABLE_NAME} (
                id INT PRIMARY KEY,
                name VARCHAR(50),
                value INT
            );
            INSERT INTO {TABLE_NAME} (id, name, value) VALUES
                (1, 'first', 42),
                (2, 'second', 43);
        """)
    with target_conn.cursor() as target_cur:
        target_cur.execute(f"""
            CREATE TABLE {TABLE_NAME} (
                id INT PRIMARY KEY,
                name VARCHAR(50),
                value INT
            );
            INSERT INTO {TABLE_NAME} (id, name, value) VALUES
                (3, 'third', 44);
        """)


    # when
    response = requests.post("http://localhost:2000/api/v1/backups/postgresSource")
    response.raise_for_status()
    response_body = response.json()
    backup_id = response_body["backup_id"]

    response = requests.post(f"http://localhost:2000/api/v1/targets/postgresTarget/backups/{backup_id}?drop=true")
    response.raise_for_status()

    # then
    with target_conn.cursor() as target_cur:
        target_cur.execute(f"SELECT id, name, value FROM {TABLE_NAME} ORDER BY value;")
        restored_rows = target_cur.fetchall()
        assert restored_rows == [
            (1, 'first', 42),
            (2, 'second', 43),
        ]
