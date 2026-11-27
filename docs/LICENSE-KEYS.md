# Key bản quyền ERG Classroom

Tutor giới hạn số máy học sinh kết nối cùng lúc theo một key ký số. Máy học sinh không cần key. Không cần Internet.

- Không có key, key sai hoặc hết hạn: dùng thử, tối đa **5 máy học sinh**. Máy vượt giới hạn không được kết nối và Tutor hiện thanh cảnh báo kèm nút **Nhập key…**.
- Khi `crates/license/erg-license.pub` còn trống, bản build **không hỏi key và không giới hạn** số máy.

## 1. Tạo cặp khoá (ERG làm một lần duy nhất)

Chạy trong thư mục gốc của repo. Thư mục chứa khoá phải có sẵn và nằm **ngoài repo**.

```powershell
cargo run -p erg-license -- keygen C:\ERG-keys\erg-license.key
```

- Lệnh lưu khoá bí mật vào file `.key` và in ra một dòng **khoá công khai**.
- Dán dòng khoá công khai vào `crates/license/erg-license.pub` (một dòng), commit, rồi build bộ cài. Từ bản đó Tutor mới kiểm tra key.
- Lệnh không ghi đè file `.key` đã có.

Giữ file `.key`:

- Bí mật: ai có file này đều tự tạo được key. Không gửi qua mail hay chat, không đưa vào repo (`*.key` đã nằm trong `.gitignore`).
- Sao lưu: mất file thì không tạo được key mới cho các bản đã phát hành; phải tạo khoá mới, phát hành lại và cấp lại key cho mọi trường.

## 2. Tạo key cho một trường

```powershell
cargo run -p erg-license -- sign C:\ERG-keys\erg-license.key 40 365 THCS Le Loi
```

Thứ tự tham số: file khoá bí mật, số máy học sinh, số ngày hiệu lực (`0` = không thời hạn), tên trường (phần còn lại của dòng lệnh).

Lệnh in ra key dạng `ERG1.<nội dung>.<chữ ký>`. Gửi nguyên chuỗi đó cho trường.

## 3. Trường nhập key

Trong Tutor: **Trợ giúp → Bản quyền…**, dán key, bấm **Kích hoạt**. Các máy đang chờ tự vào lớp, không cần khởi động lại. Key được lưu ở `license.txt` trong thư mục cấu hình của Tutor trên máy giáo viên.

## Lưu ý

- Hạn dùng chỉ kiểm tra lúc mở Tutor: key hết hạn giữa giờ dạy không làm rớt lớp.
- Key không gắn với máy: một key dùng được trên nhiều máy giáo viên. Tên trường hiện trong hộp thoại Bản quyền.
- Trước khi phát hành bản đầu tiên có khoá công khai, cấp key cho các trường đang dùng; nếu không họ sẽ bị giới hạn 5 máy.
- Mã nguồn mở (Apache-2.0): người tự build có thể bỏ kiểm tra. Key là rào cho bản cài chính thức.
