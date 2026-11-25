# Danh sách thử trên máy thật (phần việc của bạn)

Mọi mục dưới đây đã có test tự động trên máy phát triển (xem `cargo test --release --workspace`), **nhưng chưa
được thử trên phòng máy thật**. Đánh dấu ✅/❌ và ghi lại kết quả.

Chuẩn bị: tạo một Room Key (64 ký tự hex), đặt vào `room.key` của Tutor (`%APPDATA%\vn.edu.erg.classroom.tutor\room.key`)
và đưa cùng khoá đó cho học sinh qua `installer\install-student.ps1` (từng người dùng) hoặc
`installer\install-student-machine.ps1` (toàn máy, cần quyền admin).

## A. Mạng và quy mô (1 giáo viên + nhiều máy học sinh thật)
- [ ] Tutor thấy toàn bộ máy trong < 10 giây (cùng switch). Ghi số máy: ____
- [ ] Switch có IGMP snooping nhưng **không có querier**: Show còn chạy sau 5 phút không? (Kế hoạch 4.4)
- [ ] Chiếu 1080p cho ≥ 50 máy: mất gói < 1 %, trễ < 150 ms, CPU tutor < 15 %. Ghi: ____
- [ ] Máy học sinh dùng Wi-Fi: Show có giật không? (hiện chưa tự chuyển sang unicast)
- [ ] Phòng ở subnet khác: chạy `erg-gateway.exe` trên một máy, đặt `ERG_GATEWAY=host:47810` ở học sinh và tutor
      (hoặc `gateway=` trong `config.txt`) → máy xuất hiện trên Tutor?
- [ ] Gửi file 200 MB cho 50 máy < 3 phút.

## B. Lệnh cơ bản
- [ ] Khoá / mở khoá / làm đen, ảnh chụp thấy phủ kín mọi màn hình (kể cả 2 màn hình)
- [ ] Tin nhắn, Chat hai chiều, Khảo sát (cửa sổ hiện đúng, kết quả đếm đúng)
- [ ] Mở link, chạy ứng dụng, Đăng xuất / Khởi động lại / Tắt máy (cần `ERG_ALLOW_POWER=1` ở học sinh)
- [ ] Bật máy bằng Wake-on-LAN (BIOS/NIC đã bật WoL; máy phải từng online một lần để lưu MAC)
- [ ] Tắt tiếng / bật tiếng, tiếng bíp, Xoá clipboard, Lấy clipboard
- [ ] Điểm danh: hộp thoại hiện, xuất CSV mở được bằng Excel
- [ ] Bắt đầu tiết học: thanh công cụ học sinh hiện tên bài, đồng hồ đếm ngược, mục tiêu, nút Trợ giúp
- [ ] Thưởng sao hiện trên thanh công cụ
- [ ] Trưởng nhóm: Tiết học → Chia nhóm → Đặt trưởng nhóm → Cấp quyền; máy trưởng nhóm hiện ★ và nút Nhắn/Khoá/Mở nhóm, chỉ tác động máy cùng nhóm; hết thời hạn hoặc Thu hồi thì mất quyền

## C. Trình chiếu
- [ ] Chiếu màn hình / Dừng chiếu; học sinh vào muộn vẫn thấy hình sau ≤ 2 giây
- [ ] Chiếu một ứng dụng (chọn cửa sổ trong 3 giây): chỉ thấy cửa sổ đó (lưu ý: cửa sổ khác che lên vẫn bị thấy)
- [ ] Chiếu màn hình một học sinh cho cả lớp (Exhibit)
- [ ] Vẽ lên màn hình / Bảng trắng: nét vẽ **xuất hiện bên học sinh** khi đang chiếu
- [ ] Hiện phím đang gõ
- [ ] Phát video mp4 (gửi file xong mới phát; phát / tạm dừng / tua đồng bộ giữa các máy)
- [ ] Quét lần lượt, Xem / Điều khiển một máy (hình H.264 mượt ~15 fps, trễ < 100 ms? chuột + bàn phím; nếu hình không lên thì tự dùng JPEG chậm), Chụp ảnh
- [ ] Trợ lý điện thoại: Tiết học → Trợ lý trên điện thoại, mở địa chỉ trên điện thoại cùng Wi-Fi (Windows có thể hỏi tường lửa cho cổng 47900); Khoá/Mở/Nhắn hoạt động; tắt trợ lý thì địa chỉ không mở nữa
- [ ] Kiểm kê phần cứng cả lớp (CSV) mở được bằng Excel
- [ ] Ghi lại màn hình lớp rồi mở `replay.html` và file `<máy>.mp4` bằng trình phát video

## D. Kiểm soát
- [ ] Chặn web (4 chế độ) trên **Chrome, Edge, Firefox thật** — cần quyền admin ở máy học sinh (HKLM)
- [ ] Opera / Brave / Tor bị đóng khi đang bật lọc web
- [ ] Chặn ứng dụng: ứng dụng cấm bị đóng < 1 s (lưu ý: chỉ đóng sau khi mở)
- [ ] USB: chỉ đọc / cấm chạy file / chặn hết (cắm USB thật; cần admin)
- [ ] In ấn: tạm dừng / chặn và xoá hàng đợi (in thử ra máy in thật)
- [ ] Bảo vệ: chặn Control Panel, Run, Task Manager, ẩn ổ D
- [ ] Theo dõi từ khoá gõ phím: chỉ từ khớp được báo; học sinh có được thông báo không? (thanh công cụ hiện "👁 Theo dõi từ khoá" và có tin nhắn báo — kiểm tra)
- [ ] Cho phép Internet 10 phút rồi chặn lại

## E. Tệp, đánh giá, quản trị
- [ ] Quản lý tệp từ xa (xem, tạo thư mục, đổi tên, xoá); thư mục hệ thống bị từ chối
- [ ] Thu bài (`Bai1 | *.docx`) về thư mục chọn
- [ ] Giao bài kiểm tra từ file JSON (xem `examples\bai-kiem-tra.json`), học sinh làm đủ 6 dạng câu, nộp bài, chấm điểm, xuất CSV
- [ ] Hỏi miệng, chọn học sinh ngẫu nhiên
- [ ] Dòng lệnh từ xa (chỉ khi đặt `ERG_ALLOW_SHELL=1` ở máy học sinh)
- [ ] Xuất nhật ký HTML rồi in ra PDF

- [ ] Cài phần mềm từ xa (Tệp → Cài phần mềm từ xa): thử một .msi nhỏ, xem `%ProgramData%\ERGClassroom\deploy.log`; thử file bị sửa/hết hạn thì không chạy
- [ ] Terminal Server: `install-student-machine.ps1 -TerminalServer` trên Windows Server có nhiều phiên RDP → mỗi người dùng hiện thành một máy `SERVER-tên`

- [ ] Bảng tương tác: Trình chiếu → Bảng tương tác; học sinh thấy nét vẽ; bật "Học sinh được vẽ" thì học sinh vẽ được và cả lớp thấy; máy vào muộn thấy bảng; Đóng bảng thì cửa sổ học sinh đóng

## F. Cài đặt và chống tắt
- [ ] `install-student-machine.ps1` (admin) trên 1 máy: dịch vụ `ERGStudent` chạy, kill `erg-student-agent.exe` → tự bật lại sau vài giây
- [ ] Học sinh **không có quyền admin** không dừng được dịch vụ
- [ ] Gỡ cài đặt sạch (`-Uninstall`)
- [ ] `installer\ERGClassroom.msi` (build bằng `installer\build-msi.ps1`; đã build, đọc bảng, ICE sạch; **chưa cài thử**): làm theo mục "MSI học sinh" cuối file, bằng admin
- [ ] Windows 10 và 11; màn hình độ phân giải cao / 2 màn hình; DPI 125–150 %

## G. Dịch vụ SYSTEM, lệnh đặc quyền, khoá (cần admin — chưa kiểm chứng lần nào)
Cài bằng `install-student-machine.ps1` (thêm `-AllowPower` nếu muốn thử tắt máy). Tài khoản học sinh là người dùng thường.
- [ ] `icacls %ProgramData%\ERGClassroom\room.key` chỉ có SYSTEM (R) và Administrators (F); học sinh: `type room.key` bị từ chối; `icacls %ProgramData%\ERGClassroom` chỉ cho Users đọc (RX), không sửa được `config.txt`
- [ ] Máy học sinh vẫn lên Tutor sau khi cài (agent không có khoá, hỏi qua pipe `ERGClassroomAuth`); dừng dịch vụ → máy mất khỏi Tutor
- [ ] Pipe lệnh: học sinh chạy PowerShell `[IO.Pipes.NamedPipeClientStream]::new('.','ERGClassroomPriv')` rồi gửi byte rác → bị từ chối/không làm gì; **tài khoản học sinh khác (đăng nhập sau) cũng kết nối được sau khi đổi người dùng** (pipe làm mới ACL trong ≤2 giây); tài khoản không đăng nhập ở console thì `Connect()` báo Access denied
- [ ] Chặn web từ Tutor (4 chế độ) áp ngay ở Chrome/Edge/Firefox của học sinh **không cần agent chạy quyền admin**; `%ProgramData%\ERGClassroom\priv.log` ghi `Web(...) -> ok`
- [ ] USB (cắm USB thật), In ấn (máy in nối theo từng người dùng có bị tạm dừng không?), Bảo vệ (Control Panel/Run/Task Manager/ẩn ổ — kiểm tra `HKEY_USERS\<SID>` của học sinh), Chặn ứng dụng (ứng dụng của học sinh bị đóng < 1 s; **tài khoản admin khác đang đăng nhập cũng bị chặn — chấp nhận được không?**)
- [ ] Tắt máy / Khởi động lại / Đăng xuất từ Tutor: chỉ chạy khi cài `-AllowPower`; Đăng xuất (`logoff.exe <session>`) đăng xuất đúng học sinh
- [ ] Giả mạo: Tutor khác khoá (hoặc tiến trình tự viết, không có khoá) gửi `Command::Priv` → bị từ chối, `priv.log` không có dòng mới. Đổi đồng hồ Tutor lệch >5 phút → lệnh bị từ chối (xem `priv.log` im lặng; sửa giờ rồi thử lại)
- [ ] Khởi động lại dịch vụ rồi thử phát lại lệnh cũ trong 5 phút (biết trước: được chấp nhận)
- [ ] Cài kiểu từng người dùng (`install-student.ps1`) vẫn nhận lệnh chặn app/Bảo vệ từ Tutor (agent tự kiểm chữ ký); chặn web/USB sẽ báo "cần administrator" — đúng như chế độ yếu

## Giới hạn đã biết (đừng ngạc nhiên)
- Xem màn hình H.264 đi trên kết nối điều khiển (TCP-like, có thể giật nếu Wi-Fi yếu); mất gói thì hình vỡ tới keyframe kế (≤ 1 s).
- Test `erg-sysops` mouse_move thỉnh thoảng lỗi nếu có người đang di chuột trong lúc chạy test.
- Khoá phòng: Ctrl+Alt+Del không chặn được từ user-mode.
- Chặn ứng dụng chỉ đóng sau khi mở.
- Room Key: dịch vụ SYSTEM giữ key, agent không có key (hỏi qua pipe `\\.\pipe\ERGClassroomAuth`). **Cần thử**: học sinh không mở được `room.key`; agent vẫn kết nối được; dừng dịch vụ thì máy mất khỏi Tutor (đúng thiết kế); cài kiểu từng người dùng vẫn dùng key trong thư mục người dùng.
- Show chưa tự chuyển sang unicast khi mất gói nhiều (Wi-Fi).
- Sơ đồ phòng máy = kéo-thả thẻ để đổi chỗ (lưu lại), nhóm = menu Tiết học → Chia nhóm tự động / Đưa máy vào nhóm — **chưa xem bằng mắt trên giao diện thật**, hãy thử.
- Mới thêm, chưa xem bằng mắt: Soạn đề kiểm tra (Đánh giá), Bảng điểm theo đội, trưởng nhóm ★, nút EN/VI (chỉ dịch menu).
- Chưa có: Excalidraw thật (hình khối, chữ, hoàn tác) — bảng chung chỉ có nét vẽ tay; một số thông báo do Rust gửi vẫn tiếng Việt. Ngoài phạm vi theo yêu cầu: driver WFP/ETW, Credential Provider, ký số mã.

## MSI học sinh (installer/student.wxs) — ĐÃ BUILD, CHƯA CÀI THỬ

Build (không cần admin): `installer\build-msi.ps1` (cần cargo, `wix` 5.0.2 và `wix extension add -g WixToolset.Firewall.wixext/5.0.2` và `WixToolset.UI.wixext/5.0.2`) -> `installer\ERGClassroom.msi`.
Đã kiểm (chỉ đọc): các bảng Feature/Component/ServiceInstall/ServiceControl/CustomAction/Wix5FirewallException đúng; `wix msi validate` không lỗi ICE.
Cài thử (CẦN ADMIN, việc của bạn, trên 1 máy thử): `msiexec /i ERGClassroom.msi ROOM="P1" [ROOMKEY=<64 hex>] [GATEWAY=host:port] [ALLOWPOWER=1] /qn /l*v cai.log`
Kiểm: dịch vụ `ERGStudent` chạy (SYSTEM, tự khởi động); rule firewall `ERG Classroom (agent)`; `C:\ProgramData\ERGClassroom\room.key` chỉ SYSTEM/Administrators đọc được (HS không mở được);
`config.txt` có `room=`, và `allow_power=1` chỉ khi ALLOWPOWER=1; `cai.log` không lộ khoá; `msiexec /x` xoá dịch vụ, rule và thư mục ProgramData.
Không có ROOMKEY: dùng khoá mặc định có sẵn (Tutor không có `room.key` cũng dùng khoá này). ROOMKEY sai (không đủ 64 hex) phải bị từ chối. Chưa thử: nâng cấp bản mới, Windows tiếng Việt.