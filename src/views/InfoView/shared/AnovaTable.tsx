import React from 'react';
import { formatNum } from './RegressionShared';
import type { ModelBasicInfo } from './types';

export function AnovaTable({ info }: { info: ModelBasicInfo }) {
  return (
    <div className="rounded-lg border border-gray-800/50 overflow-hidden mb-2">
      <table className="w-full text-xs">
        <thead>
          <tr className="bg-[#1a1d23]">
            <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Source</th>
            <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">SS</th>
            <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">df</th>
            <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">MS</th>
          </tr>
        </thead>
        <tbody>
          <tr className="bg-[#13151a] border-t border-gray-800/30">
            <td className="px-4 py-2.5 font-mono text-white">Model</td>
            <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ss_model)}</td>
            <td className="text-right px-3 py-2.5 font-mono text-gray-300">{info.df_model}</td>
            <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ms_model)}</td>
          </tr>
          <tr className="bg-[#15171d] border-t border-gray-800/30">
            <td className="px-4 py-2.5 font-mono text-white">Residual</td>
            <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ss_residual)}</td>
            <td className="text-right px-3 py-2.5 font-mono text-gray-300">{info.df_residual}</td>
            <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ms_residual)}</td>
          </tr>
          <tr className="bg-[#13151a] border-t border-gray-800/30">
            <td className="px-4 py-2.5 font-mono text-white font-semibold">Total</td>
            <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{formatNum(info.ss_total)}</td>
            <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{info.df_total}</td>
            <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{formatNum(info.ms_total)}</td>
          </tr>
        </tbody>
      </table>
    </div>
  );
}
