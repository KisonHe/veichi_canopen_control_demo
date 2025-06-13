use clap::Parser;
use futures_util::SinkExt;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use log::error;
use log::info;
use log::warn;
use socketcan::tokio::CanFdSocket;
use socketcan::CanAnyFrame;
use socketcan::CanDataFrame;
use socketcan::CanFilter;
use socketcan::EmbeddedFrame;
use socketcan::Frame;
use socketcan::Id;
use socketcan::SocketOptions;
use socketcan::StandardId;

lazy_static! {
    static ref MOTOR_POSITIONS: [tokio::sync::Mutex<Option<i32>>; 3] = [
        tokio::sync::Mutex::new(None),
        tokio::sync::Mutex::new(None),
        tokio::sync::Mutex::new(None),
    ];
}

// Confidential File. Do not use canopen lib in public demo projects.
// Replace all sdo r/w with direct [u8;8] data
// mod canopen;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// Can interface name, e.g. can0
    #[arg(long, short)]
    can_interface: String,
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Args::parse();

    let canbus = args.can_interface.clone();
    let (mut tx, _) = {
        let canbus = CanFdSocket::open(canbus.as_str()).unwrap();
        canbus.split()
    };

    // Enable TPDO2 with 20ms interval for all three motors.
    // Spawn TPDO Readers for all three motors.
    // Spawn Control Tasks for all three motors.
    for i in 1..4u16 {
        let canbus = args.can_interface.clone();
        let id = 0x600u16 + i;
        // 0x1802 05 write 2 Byte Data: 20
        let d = [0x2Bu8, 0x02, 0x18, 0x05, 0x64, 0x00, 0x00, 0x00];
        tx.send(CanAnyFrame::Normal(
            CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
        ))
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        // 0x1802 02 Write 1 Byte Data: 255
        let d = [0x2Fu8, 0x02, 0x18, 0x02, 0xFF, 0x00, 0x00, 0x00];
        tx.send(CanAnyFrame::Normal(
            CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
        ))
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        // 0x1801 03 Write 4 Byte Data: 0x380 + i
        let d = [0x23u8, 0x02, 0x18, 0x01, 0x80 + i as u8, 0x03, 0x00, 0x00];
        tx.send(CanAnyFrame::Normal(
            CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
        ))
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // NMT Set to Operational
        let d = [0u8, i as u8];
        tx.send(CanAnyFrame::Normal(
            CanDataFrame::new(Id::Standard(StandardId::new(0).unwrap()), &d).unwrap(),
        ))
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Reader Task
        tokio::spawn(async move {
            let (_, mut rx) = {
                let canbus = CanFdSocket::open(canbus.as_str()).unwrap();
                let filter = CanFilter::new(0x380u32 + i as u32, 0x7FF);
                canbus.set_filters(&[filter]).unwrap();
                canbus.split()
            };
            loop {
                let f = rx.next().await.unwrap().unwrap();
                {
                    if f.len() != 6 {
                        continue;
                    }
                    let pos =
                        i32::from_le_bytes([f.data()[2], f.data()[3], f.data()[4], f.data()[5]]);
                    *MOTOR_POSITIONS[i as usize - 1].lock().await = Some(pos);
                }
            }
        });

        // Control Task
        let canbus = args.can_interface.clone();
        tokio::spawn(async move {
            let (mut tx, _) = {
                let canbus = CanFdSocket::open(canbus.as_str()).unwrap();
                canbus.split()
            };
            // 0x6060 0 Write 1 Byte Data: 3
            let d = [0x2Fu8, 0x60, 0x60, 0x00, 0x03, 0x00, 0x00, 0x00];
            tx.send(CanAnyFrame::Normal(
                CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // 0x60FF 0 Write 4 Byte Data: 0
            let d = [0x23u8, 0xFF, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00];
            tx.send(CanAnyFrame::Normal(
                CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // 0x6040 0 Write 2 Byte Data: 6
            let d = [0x2Bu8, 0x40, 0x60, 0x00, 0x06, 0x00, 0x00, 0x00];
            tx.send(CanAnyFrame::Normal(
                CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            // 0x6040 0 Write 2 Byte Data: 7
            let d = [0x2Bu8, 0x40, 0x60, 0x00, 0x07, 0x00, 0x00, 0x00];
            tx.send(CanAnyFrame::Normal(
                CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            // 0x6040 0 Write 2 Byte Data: 0x0f
            let d = [0x2Bu8, 0x40, 0x60, 0x00, 0x0f, 0x00, 0x00, 0x00];
            tx.send(CanAnyFrame::Normal(
                CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
            ))
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            //debug

            let control_data = [0x23u8, 0xFF, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00];
            let spd = 30000i32;
            let spd_le_bytes = spd.to_le_bytes();
            let mut d = control_data.clone();
            d[4..].copy_from_slice(&spd_le_bytes);
            tx.send(CanAnyFrame::Normal(
                CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
            ))
            .await
            .unwrap();
            //debug ends

            // The control loop.
            let mut tick = tokio::time::interval(std::time::Duration::from_millis(20));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let control_data = [0x23u8, 0xFF, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00];
            loop {
                tick.tick().await;
                // You can actully use RPDO to control the motor, but let's just stay lazy here keep using SDO...
                // Lazy is good!
                let spd = 30000i32;
                let spd_le_bytes = spd.to_le_bytes();
                let mut d = control_data.clone();
                d[4..].copy_from_slice(&spd_le_bytes);
                tx.send(CanAnyFrame::Normal(
                    CanDataFrame::new(Id::Standard(StandardId::new(id).unwrap()), &d).unwrap(),
                ))
                .await
                .unwrap();
            }
        });
    }
    drop(tx);

    loop {
        // Print motor positions, as demo.
        info!(
            "M0Pos: {:?} M1Pos: {:?} M2Pos: {:?}",
            MOTOR_POSITIONS[0].lock().await.clone(),
            MOTOR_POSITIONS[1].lock().await.clone(),
            MOTOR_POSITIONS[2].lock().await.clone()
        );
        // info!("M0Pos: {:?}", MOTOR_POSITIONS[0].lock().await.clone());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}
