# 08 — VM Pengembangan Hyper-V (Windows 11 Enterprise Evaluation, gratis)

> Mengapa: Smart App Control di PC pemilik memblokir biner yang baru dibangun, termasuk di Windows Sandbox sejak 24 Okt 2026
> (`CLAUDE.md` §5). Build dan tes otomatis berjalan di GitHub Actions. VM ini untuk **uji interaktif**:
> - klik di WebView2;
> - instal dan uninstall installer;
> - first-run di "mesin bersih".
>
> Biayanya Rp0: Hyper-V sudah ada di Windows 11 Pro, dan image evaluasi gratis 90 hari.

## 1. Prasyarat
- Windows 11 **Pro** di host (Hyper-V tidak ada di edisi Home).
- Virtualisasi CPU aktif di BIOS/UEFI (Intel VT-x / AMD-V). Cek di Task Manager → Performance → CPU → "Virtualization: Enabled".
- Ruang disk ±100 GB dan RAM ≥ 16 GB (VM memakai 8 GB).

## 2. Aktifkan Hyper-V (sekali)
Di **Terminal (Admin)**:

```powershell
Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All
```

Lalu restart. Gunakan `shutdown /r /t 0` bila restart dari menu Start ditunda.

## 3. Unduh ISO evaluasi
1. Buka <https://www.microsoft.com/evalcenter/evaluate-windows-11-enterprise>.
2. Pilih **ISO – Enterprise** 64-bit dan isi formulir (gratis).
3. Simpan, misalnya ke `D:\VMs\iso\`.

Masa berlakunya 90 hari. Setelah habis, VM bisa dibuat ulang dari checkpoint (§6) atau ISO baru.

## 4. Buat VM
Di **Terminal (Admin)**. Sesuaikan path ISO dengan nama file yang Anda unduh:

```powershell
$name = 'KinoraDev'
$iso  = 'D:\VMs\iso\Windows11_Enterprise_Eval.iso'
New-Item -ItemType Directory -Force D:\VMs | Out-Null
New-VM -Name $name -Generation 2 -MemoryStartupBytes 8GB -NewVHDPath "D:\VMs\$name.vhdx" -NewVHDSizeBytes 100GB -SwitchName 'Default Switch'
Set-VMProcessor $name -Count 6
Set-VMMemory $name -DynamicMemoryEnabled $false
Set-VMKeyProtector $name -NewLocalKeyProtector
Enable-VMTPM $name
Set-VMFirmware $name -EnableSecureBoot On -SecureBootTemplate 'MicrosoftWindows'
Add-VMDvdDrive $name -Path $iso
Set-VMFirmware $name -FirstBootDevice (Get-VMDvdDrive $name)
Start-VM $name
vmconnect.exe localhost $name
```

TPM dan Secure Boot adalah syarat Windows 11. Tekan sembarang tombol saat muncul "Press any key to boot from CD".

## 5. Instal dan siapkan Windows di VM
1. Pilih **Windows 11 Enterprise Evaluation**, instalasi *Custom*, lalu ke disk kosong.
2. Saat OOBE, gunakan **akun lokal** (opsi *Domain join instead* / *Sign-in options → Offline account*). Jangan memakai akun Microsoft pribadi.
3. Matikan opsi diagnostik dan iklan saat ditanya. Jalankan Windows Update sampai bersih.
4. Pasang alat pengembangan (semua installer resmi bertanda tangan):

   | Alat | Untuk |
   |---|---|
   | Git | Mengkloning repo |
   | Node.js 24 LTS | UI, skrip |
   | Rust (`rustup-init.exe`) | Shell Tauri; `rustup` membaca `rust-toolchain.toml` |
   | Visual Studio Build Tools, workload *Desktop development with C++* | MSVC + Windows SDK |

   WebView2 sudah terpasang di Windows 11.
5. Kloning repo **di dalam VM** (`git clone https://github.com/Rvl08/kinora.git`). Jangan memetakan folder host secara read-write ke VM, agar VM tetap terisolasi dari file pribadi Anda.
6. **Smart App Control di dalam VM** adalah keputusan Anda, karena ini mesin Anda sendiri. Claude tidak mengubahnya.
   - VM ini terpisah dari host: pengaturan di dalamnya tidak menyentuh SAC di PC Anda.
   - Bila SAC aktif di VM, biner yang baru dibangun juga akan diblokir di sana.
   - Mengubahnya: *Windows Security → App & browser control → Smart App Control settings*. Pembaruan Windows 2026 memungkinkan SAC dinyalakan kembali tanpa instal ulang.
7. Opsional: pasang **Claude Desktop** di VM agar Claude bisa menjalankan uji interaktif langsung di sana.

## 6. Checkpoint
Setelah §5 selesai, buat checkpoint bersih agar VM bisa dikembalikan dalam hitungan detik:

```powershell
Checkpoint-VM -Name KinoraDev -SnapshotName 'clean-tools'
```

Untuk uji installer di "mesin bersih", kembalikan checkpoint sebelum setiap percobaan:

```powershell
Restore-VMCheckpoint -VMName KinoraDev -Name 'clean-tools' -Confirm:$false
```

## 7. Batasan
- **GPU:** VM standar tidak punya akselerasi GPU, dan WebView2 merender dengan perangkat lunak. Itu cukup untuk Sprint B (layar awal, dialog, simpan/buka). Uji preview WebGPU/NVENC butuh *GPU partitioning* (GPU-P) dan akan dibahas saat dibutuhkan.
- **Lisensi evaluasi:** hanya untuk pengujian, bukan pemakaian produksi.
