// Trunk の初期化フック: wasm のダウンロード進捗を表示する
export default function () {
  const fill = () => document.getElementById("fill");
  const status = () => document.getElementById("status");
  return {
    onStart: () => { status().textContent = "ゲームデータを読み込み中…"; },
    onProgress: ({ current, total }) => {
      if (total) {
        const pct = Math.min(100, (current / total) * 100);
        fill().style.width = pct.toFixed(1) + "%";
        status().textContent = `ゲームデータを読み込み中… ${(current / 1048576).toFixed(1)} / ${(total / 1048576).toFixed(1)} MB`;
      } else {
        status().textContent = `ゲームデータを読み込み中… ${(current / 1048576).toFixed(1)} MB`;
      }
    },
    onComplete: () => { fill().style.width = "100%"; status().textContent = "世界を生成しています…"; },
    onSuccess: () => {},
    onFailure: (error) => {
      const e = document.getElementById("error");
      e.style.display = "block";
      e.textContent = "起動に失敗しました。WebGL2 に対応したブラウザ (Chrome / Edge / Firefox / Safari 最新版) でお試しください。 " + error;
    },
  };
}
