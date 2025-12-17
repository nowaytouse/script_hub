// VVebo粉丝列表修复脚本 - 修复版
// 原作者: suiyuran
// 修复: 增加安全检查防止passing card error

let url = $request.url;

if (url.includes("selffans")) {
  try {
    let data = JSON.parse($response.body);
    
    // 安全检查
    if (!data || !data.cards || !Array.isArray(data.cards)) {
      console.log("[VVebo粉丝修复] 警告: cards数据无效");
      $done({});
      return;
    }
    
    let cards = data.cards.filter((card) => card && card.itemid !== "INTEREST_PEOPLE2");
    $done({ body: JSON.stringify({ ...data, cards }) });
  } catch (e) {
    console.log("[VVebo粉丝修复] 解析错误: " + e.message);
    $done({});
  }
} else {
  $done({});
}
