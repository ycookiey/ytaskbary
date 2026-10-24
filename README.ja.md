# ytaskbary

[English](README.md)

Windows 11のタスクバーを隠し、スタートメニューが前面にある間だけ表示するツール。タスクトレイに常駐する。Windowsの設定でタスクバーを自動的に隠すだけでは、マウスポインターを画面の端に動かすとタスクバーが表示されるが、ytaskbaryはそのときも隠したままにする。

## 特徴

- 全モニターのタスクバーを隠す
- 既定では、スタートメニューが前面にある間はタスクバーを表示し、別のウィンドウに切り替わると隠す
- 既定ではキー入力を監視せず、一定の間隔で動く処理も無いので、ゲームが前面にある間もキー入力を遅らせない
- タスクトレイのメニュー:
  - Enabled: チェックを外すと、タスクバーを隠すのをやめる
  - Start at sign-in: サインインしたときにytaskbaryを起動する
  - Exit: 終了し、タスクバーを再び表示する

## インストール

[Scoop](https://scoop.sh/)でインストールするか、[Releases](https://github.com/ycookiey/ytaskbary/releases)から`ytaskbary.exe`をダウンロードする。Scoopでインストールするコマンドは次のとおり。

```powershell
scoop bucket add yscoopy https://github.com/ycookiey/yscoopy
scoop install yscoopy/ytaskbary
```

## 使い方

1. Windowsの設定の「個人用設定」の「タスク バー」で、タスクバーを自動的に隠す設定をオンにする。オンにすると、最大化したウィンドウがタスクバーの所まで広がる
2. `ytaskbary.exe`を起動する。タスクバーが隠れる
3. メニューを開くには、スタートメニューを開いてタスクバーを表示し、タスクトレイのytaskbaryのアイコンをクリックする。アイコンは、隠れているインジケーター(^)の中にあることがある

スタートメニューを開いてもタスクバーが表示されないときは、ytaskbaryを終了し、`--keyboard-hook`を付けて起動する。`--keyboard-hook`を付けると、Winキーだけを押して離したときにタスクバーを表示し、ほかの方法でスタートメニューを開いたときは表示しない。キー入力の監視にはWindowsの低レベルキーボードフックを使い、フックを付けている間は、キーを押すたびにWindowsがytaskbaryの処理を待つ。そのため、ゲームが前面にある間はフックを外す。ゲームかどうかは、前面のアプリの実行ファイルが、WindowsがXbox Game Barのために記録しているゲームの一覧に載っているかで判定する。「Start at sign-in」は、チェックを付けたときの起動の方式を引き継ぐ。

## 補足

- 動作確認は、Windows 11で、タスクバーを1台のモニターにだけ表示した環境のみ
- 「Exit」以外で終了したとき(タスクマネージャーでの終了など)は、タスクバーが隠れたまま残る。ytaskbaryをもう一度起動し、「Exit」で終了する
- ログの保存先: `%LOCALAPPDATA%\ytaskbary\ytaskbary.log`

## アンインストール

1. メニューの「Start at sign-in」にチェックが付いていれば、クリックしてチェックを外す
2. メニューの「Exit」をクリックして終了する
3. Scoopでインストールした場合は`scoop uninstall ytaskbary`を実行する。Releasesからダウンロードした場合は`ytaskbary.exe`を削除する
4. ログのあるフォルダ`%LOCALAPPDATA%\ytaskbary`を削除する

## ビルド

RustのMSVCのツールチェーンでビルドする。

```powershell
cargo build --release
.\target\release\ytaskbary.exe
```

## ライセンス

[MIT License](LICENSE)
