//! Explicit, copy-only migration from a user-selected legacy data folder.
//! The original folder is never modified. Installation happens before opening the new DB.
use std::path::Path;
use rusqlite::{Connection,OpenFlags};
const STAGE: &str = "legacy-migration";
fn error(_: impl std::fmt::Display) -> String { "以前のデータを移行できません。保存先の権限・空き容量と、旧アプリが終了していることを確認してください".into() }
fn copy_tree(source: &Path, dest: &Path) -> Result<(),String> {
    let metadata=match std::fs::symlink_metadata(source) {
        Ok(m) => m,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(error(e)),
    };
    if metadata.file_type().is_symlink() { return Err("保存先外へのリンクを含むデータは移行できません".into()); }
    if metadata.is_dir() {
        std::fs::create_dir_all(dest).map_err(error)?;
        for entry in std::fs::read_dir(source).map_err(error)? { let entry=entry.map_err(error)?;copy_tree(&entry.path(),&dest.join(entry.file_name()))?; }
    } else if metadata.is_file() {
        std::fs::copy(source,dest).map_err(error)?;
        let after=std::fs::metadata(source).map_err(error)?;
        if after.len()!=metadata.len() || after.modified().ok()!=metadata.modified().ok() { return Err("コピー中に旧データが変更されました。旧アプリを終了してからやり直してください".into()); }
    } else { return Err("移行できない種類のファイルが含まれています".into()); }
    Ok(())
}
fn empty(root: &Path) -> Result<bool,String> {
    let path=root.join(crate::commands::DB_FILE);
    if !path.exists() { return Ok(true); }
    let db=Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    let count:i64=db.query_row("SELECT count(*) FROM meetings",[],|r|r.get(0)).map_err(error)?;
    Ok(count==0)
}
pub fn prepare(source: &Path, root: &Path) -> Result<(),String> {
    if !empty(root)? { return Err("移行先に議事録があります。既存データを保護するため上書きしません".into()); }
    let source=source.canonicalize().map_err(error)?;
    let destination=root.canonicalize().map_err(error)?;
    if source==destination || source.starts_with(&destination) || destination.starts_with(&source) { return Err("以前の保存先と新しい保存先は別のフォルダーを選んでください".into()); }
    if !source.join(".factory-app-data").is_file() { return Err("minutesの保存フォルダーを選んでください".into()); }
    let stage=root.join(STAGE);
    if stage.exists() { return Err("移行用コピーが既にあります。再起動して移行状態を確認してください".into()); }
    std::fs::create_dir(&stage).map_err(error)?;
    let result=(|| {
        let db=Connection::open_with_flags(source.join(crate::commands::DB_FILE),OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
        let integrity:String=db.query_row("PRAGMA quick_check",[],|r|r.get(0)).map_err(error)?;
        if integrity!="ok" { return Err("以前のデータベースを正常に読めません。元の保存先は変更していません".into()); }
        db.backup("main",&stage.join(crate::commands::DB_FILE),None).map_err(error)?;
        copy_tree(&source.join("audio"),&stage.join("audio"))?;
        copy_tree(&source.join("work"),&stage.join("work"))?;
        copy_tree(&source.join("usage.dat"),&stage.join("usage.dat"))?;
        let snapshot=Connection::open(stage.join(crate::commands::DB_FILE)).map_err(error)?;
        let rows:Vec<(i64,Option<String>,Option<String>)>=snapshot.prepare("SELECT id,audio_path,pcm_path FROM meetings").map_err(error)?
            .query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(error)?.collect::<Result<_,_>>().map_err(error)?;
        let relocate=|old:Option<String>|->Result<Option<String>,String> {
            old.filter(|p|!p.is_empty()).map(|path| {
                let parsed=Path::new(&path);
                let normalized=parsed.canonicalize().unwrap_or_else(|_| parsed.parent().and_then(|p|p.canonicalize().ok()).map(|p|p.join(parsed.file_name().unwrap_or_default())).unwrap_or_else(||parsed.to_path_buf()));
                let relative=normalized.strip_prefix(&source).map_err(|_|"以前の保存先の外にある音声は移行できません".to_string())?;
                if relative.components().any(|c|matches!(c,std::path::Component::ParentDir)) { return Err("不正な保存パスが含まれています".into()); }
                Ok(root.join(relative).to_string_lossy().into_owned())
            }).transpose()
        };
        for (id,audio,pcm) in rows {
            snapshot.execute("UPDATE meetings SET audio_path=?2,pcm_path=?3 WHERE id=?1",rusqlite::params![id,relocate(audio)?,relocate(pcm)?]).map_err(error)?;
        }
        // Completed chunk checkpoints and edits are retained; normal startup recovers interrupted jobs.
        drop(snapshot);
        std::fs::write(stage.join("prepared"),b"minutes legacy migration v1").map_err(error)?;
        Ok(())
    })();
    if result.is_err() { let _=std::fs::remove_dir_all(&stage); }
    result
}
pub fn apply_pending(root: &Path) -> Result<(),String> {
    let stage=root.join(STAGE);
    if !stage.exists() { return Ok(()); }
    let installing=stage.join("installing").exists();
    if !installing {
        if !stage.join("prepared").is_file() { return Err("未完了の移行コピーがあります。元のデータは変更していません".into()); }
        if !empty(root)? { return Err("移行先に新しい議事録があるため、旧データを上書きしません".into()); }
        for name in ["audio","work"] {
            let target=root.join(name);
            if target.exists() && std::fs::read_dir(&target).map_err(error)?.next().is_some() { return Err("移行先に録音データがあるため上書きしません".into()); }
        }
        std::fs::create_dir_all(stage.join("empty-destination-backup")).map_err(error)?;
        // Marker precedes each rename; on interruption the next launch resumes installation.
        std::fs::write(stage.join("installing"),b"v1").map_err(error)?;
    }
    for name in [crate::commands::DB_FILE,"minutes.sqlite3-wal","minutes.sqlite3-shm"] {
        let old=root.join(name);let backup=stage.join("empty-destination-backup").join(name);
        if old.exists() && !backup.exists() && stage.join(crate::commands::DB_FILE).exists() { std::fs::rename(old,backup).map_err(error)?; }
    }
    for name in ["audio","work",crate::commands::DB_FILE] {
        let source=stage.join(name);let target=root.join(name);
        if source.exists() {
            if target.is_dir() { std::fs::remove_dir(&target).map_err(error)?; }
            std::fs::rename(source,target).map_err(error)?;
        }
    }
    if stage.join("usage.dat").exists() {
        // Merge copies only: never write back into the selected legacy folder.
        let _=crate::plan::Ledger::new(vec![Box::new(crate::plan::FileSlot(root.join("usage.dat"))),Box::new(crate::plan::FileSlot(stage.join("usage.dat")))]);
    }
    std::fs::rename(stage,root.join("legacy-migration-completed")).map_err(error)?;
    Ok(())
}

pub fn cancel_prepared(root: &Path) -> Result<(),String> {
    let stage=root.join(STAGE);
    if stage.join("prepared").exists() && !stage.join("installing").exists() {
        std::fs::remove_dir_all(stage).map_err(error)?;
    }
    Ok(())
}
