-- TEXT type is used as a primary key since sqlite can't store 64 bit unsigned integer
-- https://www.sqlite.org/datatype3.html
CREATE TABLE "backup_metadata" (
	"backup_id" TEXT PRIMARY KEY,
	"created_at" TEXT NOT NULL,
	"backup_size_bytes" TEXT NOT NULL,
	"backup_target_kind" TEXT NOT NULL,
	"backup_target_name" TEXT NOT NULL,
	"backup_type" TEXT NOT NULL,
	"backup_format" TEXT NOT NULL
)
