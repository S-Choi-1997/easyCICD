use std::collections::HashSet;
use std::time::Duration;
use anyhow::Result;
use tokio::net::TcpListener;
use tokio::time::sleep;
use tracing::{info, warn};
use sqlx::SqlitePool;

/// Port Scanner Worker
/// Scans port ranges every 60 seconds and updates port_allocations table
pub async fn run_port_scanner(pool: SqlitePool) -> Result<()> {
    info!("Port scanner worker started");

    loop {
        sleep(Duration::from_secs(60)).await;  // 1분마다 스캔

        if let Err(e) = scan_and_update_ports(&pool).await {
            warn!("Port scan failed: {}", e);
        }
    }
}

async fn scan_and_update_ports(pool: &SqlitePool) -> Result<()> {
    info!("Starting port scan...");

    // 1. Application 포트 범위 스캔 (10000-14999)
    scan_port_range(pool, 10000, 14999, "application").await?;

    // 2. Container 포트 범위 스캔 (15000-19999)
    scan_port_range(pool, 15000, 19999, "container").await?;

    info!("Port scan completed");
    Ok(())
}

async fn scan_port_range(
    pool: &SqlitePool,
    start: u16,
    end: u16,
    port_type: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    // DB에서 현재 할당된 포트 조회
    let allocated_ports = get_allocated_ports(pool, port_type).await?;

    for port in start..=end {
        // 이미 프로젝트/컨테이너에 할당된 포트는 스킵
        if allocated_ports.contains(&(port as i32)) {
            continue;
        }

        // 포트 사용 여부 체크
        let is_available = check_port_available(port).await;

        if !is_available {
            // 외부 프로그램이 사용 중 → DB에 기록
            sqlx::query(
                r#"
                INSERT INTO port_allocations (port, port_type, status, owner_type, last_checked_at)
                VALUES (?, ?, 'used_by_system', 'external', ?)
                ON CONFLICT(port) DO UPDATE SET
                    last_checked_at = excluded.last_checked_at
                "#
            )
            .bind(port as i32)
            .bind(port_type)
            .bind(&now)
            .execute(pool)
            .await?;
        } else {
            // 사용 가능 → 기존 'used_by_system' 레코드 삭제 (해제됨)
            sqlx::query(
                "DELETE FROM port_allocations WHERE port = ? AND status = 'used_by_system'"
            )
            .bind(port as i32)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

async fn get_allocated_ports(pool: &SqlitePool, port_type: &str) -> Result<HashSet<i32>> {
    let ports: Vec<i32> = if port_type == "application" {
        // projects 테이블에서 blue_port, green_port 조회
        sqlx::query_scalar(
            "SELECT blue_port FROM projects UNION SELECT green_port FROM projects"
        )
        .fetch_all(pool)
        .await?
    } else {
        // containers 테이블에서 port 조회
        sqlx::query_scalar(
            "SELECT port FROM containers WHERE port IS NOT NULL"
        )
        .fetch_all(pool)
        .await?
    };

    Ok(ports.into_iter().collect())
}

async fn check_port_available(port: u16) -> bool {
    match TcpListener::bind(format!("0.0.0.0:{}", port)).await {
        Ok(_) => true,   // 바인딩 성공 = 사용 가능
        Err(_) => false, // 바인딩 실패 = 사용 중
    }
}
