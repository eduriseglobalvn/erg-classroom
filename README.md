# ERG Classroom

Phần mềm quản lý lớp học máy tính mã nguồn mở cho Windows: giáo viên (Tutor) xem, chiếu màn hình, khoá máy, gửi file, chặn web/ứng dụng, giao bài kiểm tra cho cả phòng máy (mục tiêu ≥ 50 máy học sinh). Viết lại từ đầu bằng Rust + Tauri 2 + React 19 (giao diện Fluent). Kế hoạch chi tiết và tiến độ: `F:\ERGSupport\PLAN.md` (mục 14).

> Trạng thái: toàn bộ đã có test tự động, **nhưng chưa thử trên phòng máy thật**. Danh sách cần thử: [TEST-MAY-THAT.md](TEST-MAY-THAT.md).

## Kiến trúc

- Tutor và Student nói chuyện qua QUIC (TLS 1.3) bằng giao thức riêng (`postcard`), tìm nhau qua discovery UDP (multicast/broadcast/unicast) hoặc `erg-gateway` cho subnet khác.
- Chiếu màn hình (Show): chụp DXGI -> mã hoá H.264 (Media Foundation) -> chia gói + FEC + AES-GCM -> UDP multicast; học sinh giải mã bằng WebCodecs.
- Máy học sinh: dịch vụ `ERGStudent` (SYSTEM) giữ Room Key và tự bật lại agent giao diện trong phiên người dùng; agent hỏi dịch vụ qua named pipe, không tự giữ key.

## Bản đồ thư mục

| Thư mục | Vai trò |
|---|---|
| `crates/proto` | Kiểu thông điệp, cổng, mã hoá khung |
| `crates/net` | QUIC, chứng chỉ, xác thực Room Key, discovery, auth oracle |
| `crates/authpipe` | Named pipe giữa agent và dịch vụ SYSTEM |
| `crates/tutor-core` | Logic Tutor: lớp học, điểm danh, báo cáo, nhóm, trưởng nhóm, bảng, trợ lý điện thoại |
| `crates/student-core` | Logic học sinh: kiểm tra đầu vào, lệnh, tự dừng Show khi mất tutor |
| `crates/show`, `capture`, `encode`, `mp4` | Chia gói/FEC, chụp màn hình, H.264, ghi MP4 (replay) |
| `crates/broadcast` | Chiếu màn hình/ứng dụng, Xem/Điều khiển, Replay |
| `crates/policy` | Chặn web và ứng dụng |
| `crates/sysops` | Âm thanh, clipboard, USB, in ấn, bảo vệ, SendInput, pin/Wi-Fi |
| `crates/assess` | Bài kiểm tra 6 dạng câu, chấm điểm, hỏi miệng |
| `crates/deploy` | Cài phần mềm từ xa (chữ ký HMAC + SHA-256) |
| `crates/gateway` | `erg-gateway`: cầu nối discovery qua subnet |
| `crates/sim` | `student-sim`: giả lập N học sinh để thử Tutor |
| `apps/tutor` | App Tutor (Tauri + React) |
| `apps/student-agent` | App học sinh hiển thị (cửa sổ Show, khoá, chat...) |
| `apps/student-svc` | `erg-student`: runtime học sinh (console/nền) |
| `apps/student-watchdog` | `erg-student-watchdog`: dịch vụ Windows `ERGStudent` |
| `installer/` | `install-student.ps1` (theo người dùng), `install-student-machine.ps1` (toàn máy, admin), `student.wxs` + `build-msi.ps1` (một MSI `ERGClassroom.msi`, chọn Student hoặc Teacher khi cài) |
| `examples/` | Ví dụ bài kiểm tra JSON |

## Build và test

```powershell
cargo build --release --workspace
cargo test --release --workspace        # test Rust toàn workspace

cd apps\tutor
npm install
npx tsc --noEmit                         # kiểm tra kiểu
# test TS là script node thuần (không dùng vitest):
foreach ($t in Get-ChildItem tests\*.test.ts) { node --experimental-strip-types $t.FullName }
npm run tauri dev                        # chạy Tutor (hoặc: npm run tauri build)

cd ..\student-agent                      # tương tự
node --experimental-strip-types tests\i18n.test.ts
```

Cần Rust stable, Node 22+ (type-stripping), WebView2. Test `erg-sysops` mouse_move có thể lỗi nếu bạn đang di chuột lúc chạy.

## Chạy thử nhanh (1 máy)

```powershell
# Cửa sổ 1: 50 học sinh giả (khoá dev cố định, chỉ để thử)
cargo run --release -p erg-sim --bin student-sim -- 50
# Cửa sổ 2: Tutor
cd apps\tutor; npm run tauri dev
```

Tutor tự tìm thấy 50 máy giả. Học sinh thật: đặt cùng Room Key (64 ký tự hex) vào `room.key` của Tutor (`%APPDATA%\vn.edu.erg.classroom.tutor\room.key`) và cài bằng `installer\install-student.ps1` (xem TEST-MAY-THAT.md).

## Mô hình bảo mật

- **Room Key**: bí mật chung của phòng, gắn vào phiên TLS; sai khoá thì không kết nối được. Với cài toàn máy, dịch vụ SYSTEM giữ key; agent chỉ hỏi "đây có phải bằng chứng của giáo viên?" qua named pipe.
- **Chứng chỉ ghim (pinned cert)**: học sinh chỉ tin chứng chỉ Tutor đã ghim, không tin CA hệ thống.
- Luồng Show mã hoá AES-GCM; gói giả mạo bị loại. Gửi file kiểm SHA-256, chống tên file độc. Cài từ xa yêu cầu chữ ký HMAC phủ tên, hash, tham số, hạn dùng. Dòng lệnh từ xa tắt mặc định (`ERG_ALLOW_SHELL=1`).
- **Minh bạch với học sinh**: học sinh luôn thấy khi đang bị xem/điều khiển/theo dõi từ khoá (thanh công cụ + thông báo); chỉ hoạt động trong mạng trường; Tutor ghi nhật ký thao tác.

## Giới hạn đã biết

- Chưa thử trên LAN/phòng máy thật, chưa thử 50 máy thật; Show chưa tự chuyển sang unicast khi Wi-Fi mất gói nhiều; IGMP snooping không querier có thể làm Show dừng.
- Chặn web/USB cần quyền admin (HKLM), không phủ hết trình duyệt lạ. Chặn ứng dụng chỉ đóng sau khi mở, không chặn khởi chạy.
- Không chặn được Ctrl+Alt+Del từ user-mode; học sinh có quyền admin có thể tắt bản cài theo người dùng.
- Dịch vụ `ERGStudent`, cài toàn máy, MSI (`student.wxs`) mới được viết/kiểm cú pháp, chưa chạy thật; chưa ký số mã. Không có driver WFP/ETW/Credential Provider (ngoài phạm vi).
- Xem/Điều khiển dùng H.264 ~15 fps (quay về JPEG ~2 khung/giây nếu thiếu WebCodecs); chiếu ứng dụng vẫn thấy cửa sổ khác che lên.
- Bảng tương tác là canvas nét vẽ tay, không phải Excalidraw. Trợ lý điện thoại là HTTP thuần trong LAN (token ngẫu nhiên, không TLS). Gateway chỉ UDP/LAN định tuyến, không xuyên NAT.
- Giao diện mới (sơ đồ kéo-thả, soạn đề, bảng điểm đội, trưởng nhóm, i18n vi/en) chưa xem bằng mắt trên máy thật; một số chuỗi do Rust gửi chưa dịch.

## Key bản quyền

Tutor giới hạn số máy học sinh theo key bản quyền ký số (không cần Internet). Cách tạo khoá và tạo key cho trường: [docs/LICENSE-KEYS.md](docs/LICENSE-KEYS.md).

## Pháp lý

Viết lại từ đầu (clean-room): không dùng mã, giao diện, tài nguyên hay tên thương mại của phần mềm khác (kể cả NetSupport); chỉ tham khảo danh mục tính năng chung. Giấy phép: **Apache-2.0** (xem [LICENSE](LICENSE)). Dùng trong mạng trường, với sự biết của học sinh và phụ huynh theo quy định của trường.
