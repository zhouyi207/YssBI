<div align="center">

# YssBI

**基于 Tauri 的桌面端数据分析与可视化应用**

以**节点图编辑器**为核心交互形态，通过拖拽和连接节点构建统计分析与计量经济学工作流。

<br />

<img src="imgs/demo.png" alt="YssBI 界面预览" width="800" />

</div>

---

## 功能概览

- **图编辑器** — 在画布上组合节点（数据导入、清洗、统计建模、绘图），通过连线构建分析流程
- **数据管理** — 支持 CSV、Parquet、Excel、SQLite、PostgreSQL、MySQL 多种数据源，提供数据表格浏览（虚拟滚动）、列统计与分布、单元格编辑
- **统计分析** — OLS、2SLS、LIML、二元选择模型（Logit/Probit）、VAR/VEC、Panel FE/RE/FD/LSDV、DID、Prais-Winsten、ACF/PACF、序列相关检验（DW/BG/LB）、假设检验
- **可视化** — 散点图、折线图、直方图、KDE、ECDF、条形图、相关图、平行坐标图、残差图、脉冲响应图、DID 事件研究图
- **项目管理** — 项目注册、收藏、多图管理，支持图文件夹分类（Event/Function）
- **国际化** — 中英文界面切换
- **主题系统** — 可配置的外观、编辑器与窗口布局主题

## 快速开始

```bash
# 安装依赖
npm install

# 开发
npm run tauri dev

# 构建
npm run tauri build
```

## License

未发布，开发阶段。
