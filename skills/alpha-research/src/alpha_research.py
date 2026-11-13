"""
Alpha Research CLI Entry Point
論據格式: <signal_formula> [--universe <universe>] [--horizon <days>] [--holding <period>]
"""

import typer
from pathlib import Path
from typing import Optional
from rich.console import Console

app = typer.Typer(help="Alpha Research - 量化因子與 Alpha 預測信號研究")
console = Console()


@app.command()
def analyze(
    signal_formula: str = typer.Argument(..., help="信號因子名稱，如 momentum_12_1、vol_reversal_5d"),
    universe: str = typer.Option("crypto_top50", "--universe", "-u", help="標的資產池代號 (crypto_top50, sp500, futures_liquid)"),
    horizon: int = typer.Option(5, "--horizon", "-h", help="預測收益週期（天數），有效範圍：1-63"),
    holding: int = typer.Option(5, "--holding", "-H", help="持倉期（天數）"),
    data_path: str = typer.Option(..., "--data-path", "-d", help="本地行情特徵資料檔路徑（Parquet 或 CSV）"),
    target_ic: float = typer.Option(0.03, "--target-ic", "-t", help="預期最低平均資訊係數（IC Mean 門檻）"),
    output_dir: str = typer.Option("reports/alpha/", "--output-dir", "-o", help="因子分析報告與評估數據落盤目錄"),
):
    """
    執行 Alpha 因子分析：IC/ICIR 計算、IC 衰減分析、周轉率與扣費檢驗
    """
    from core.ic_icir import AlphaResearchPipeline
    
    # 步驟 0：前置安全檢查
    pipeline = AlphaResearchPipeline(
        signal_name=signal_formula,
        universe=universe,
        data_path=data_path,
        horizon=horizon,
        holding_period=holding,
        target_ic=target_ic,
        output_dir=output_dir,
    )
    
    try:
        report_path = pipeline.run()
        console.print(f"[green]✓ 分析完成，報告已輸出至: {report_path}[/green]")
    except ValueError as e:
        console.print(f"[red]✗ 驗證失敗: {e}[/red]")
        raise typer.Exit(1)
    except Exception as e:
        console.print(f"[red]✗ 執行錯誤: {e}[/red]")
        raise typer.Exit(1)


if __name__ == "__main__":
    app()
