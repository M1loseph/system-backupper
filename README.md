# System backuper

This project has one goal - connect to the processes that persist data in phd-website system and back it up to other location.

## Fetching data from dependencies

### MongoDB

1. It requres [mongodump](https://www.mongodb.com/docs/database-tools/mongodump/) utility. It is used to dump database state to a binary archive.
2. It also requires [mongorestore](https://www.mongodb.com/docs/database-tools/mongorestore/). This utility on the other hand is used to restore the data from a backup.

### Prometheus

1. It connect to prometheus via its [rest api](https://prometheus.io/docs/prometheus/latest/querying/api/)
