# 🎮 Enshrouded Mod Loader (EML): User 4 (GPORTAL Server Admin Guide)

This guide provides step-by-step instructions for GPORTAL server administrators managing dedicated multiplayer servers and installing mods using the **Enshrouded Mod Loader (EML)**.

> 💡 **Looking for EML Mods?**
> Be sure to download the standalone **EML Ecosystem Directory** from the **Files** tab on Nexus Mods alongside this guide for a list of mods available for download.

---

⚡ **TL;DR:** Configure GPORTAL web settings for EML. Stop server. Connect via FileZilla FTP. Back up remote directory. Upload `dbghelp.dll` if missing & drop mods into `mods`. Restart server.

---

### Part 1: GPORTAL Web Control Panel Setup

1. Go to GPORTAL at [https://www.g-portal.com/](https://www.g-portal.com/) and click **Login**.
2. Sign in with your GPORTAL username and password.
3. Click **My Servers** on the left menu, then select the server you wish to mod.
4. Enable mods through **Basic Settings**:
   * **a.** Click **Basic Settings** on the left.
   * **b.** Click **Enshrouded Mods** at the top.
   * **c.** Change **Mod Choice** from `No Mod` to `Enshrouded Mod Loader`.
   * **d.** Click **Save** at the bottom.
5. While editing your server settings, click the slider bar at the top of the page to **temporarily stop the server**. *(You will restart it via this same slider once your file transfers are complete).*

---

### Part 2: Connecting via FTP (FileZilla Setup)

> 💡 **Note on Tools:** While GPORTAL provides a basic browser "File Manager", it does not allow file deletions. Setting up an FTP manager grants complete editing and cleanup control over your server files. These steps are tested using **FileZilla Portable (ZIP version)**.

1. Download the portable zip version of FileZilla (`FileZilla_3.71.0_win64.zip` or similar) from [FileZilla Client Downloads](https://filezilla-project.org/download.php?show_all=1). Using the Zip version is recommended as it is lightweight, portable, and avoids bundled adware or registry modifications associated with the `.exe` installer.
2. Extract the downloaded zip file (`Right-click > Extract All...`) to your preferred directory.
3. Open `filezilla.exe`.
4. On your GPORTAL server dashboard page, click **Status** on the left, then scroll down to the **Access Data > FTP** section. Click the copy icon next to the **FTP Link** (which follows the format `ftp://username:password@ip:port`).
5. In FileZilla, right-click the top **Host** field and paste (`CTRL + V`), then click **Quickconnect**.
   * *Note 1:* If prompted about connecting to an unsecured FTP, click **OK**. This warning occurs because GPORTAL supplies a standard FTP address rather than an encrypted FTPS endpoint.
   * *Note 2:* Choose your preferred password-saving preference when prompted by FileZilla.
6. Once connected, FileZilla will display your server files in the bottom-right pane (referred to below as the **Enshrouded Directory**).

---

### Part 3: Backing Up Your Server

1. In FileZilla's top-left pane, navigate to the local folder on your computer where you want to store your server backup.
2. (Optional) Right-click and select **Create Directory and enter it** to organize your backup workspace.
3. In the bottom-right pane (**Enshrouded Directory**), press `CTRL + A` (or use `SHIFT + Click`) to select all server files.
4. Right-click the selection and click **Download** to pull a full backup locally.

---

### Part 4: Installing & Managing Mods

1. **Install the Loader:** Confirm that GPORTAL has populated the latest `dbghelp.dll` file from [EML Releases](https://github.com/Brabb3l/kfc-parser/releases). If missing, upload `dbghelp.dll` directly into the root of your **Enshrouded Directory**.
2. **Verify Mods Folder:** Confirm that a folder named `mods` exists inside your main **Enshrouded Directory**. If not, create it.
3. **Upload Mods:**
   * **a.** Download a mod from a trusted site (such as NexusMods) and extract it locally using [7-Zip](https://www.7-zip.org/).
   * **b.** Verify that the unpacked folder contains `mod.json` and a `src` folder at the top level.
   * **c.** Drag and drop that top-level mod folder into the remote `mods` directory via FileZilla.
4. **Restart Server:** Once mods are added, return to your GPORTAL control panel and click the top slider bar to **restart the server**.
5. **Test In-Game:** Launch Enshrouded normally. The mod loader runs automatically in the background on the dedicated server. Join your server and verify that the installed mods are functioning properly.

---

### 🙏 Credits & Acknowledgments

A massive thank you to **Brabb3l** for creating and maintaining the **Enshrouded Mod Loader (EML)** and the `kfc-parser` toolkit. Without their hard work reverse-engineering game assets and building the Lua execution runtime, community data mining and modding for Enshrouded wouldn't be where it is today.

* 📂 **GitHub Repository:** [Brabb3l/kfc-parser](https://github.com/Brabb3l/kfc-parser)
* 📖 **Official EML Documentation:** [brabb3l.github.io/kfc-parser](https://brabb3l.github.io/kfc-parser/eml/index.html)
