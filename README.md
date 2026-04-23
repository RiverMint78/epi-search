# epi-search

> [!TIP]
>
> 这是一个*玩具*项目, 所产出的 ***数学发现*** 可以在本仓库根目录的 `mathematical-discoveries.md` 中找到 :)

基于简单数学常数近似的表达式搜索器, 算法可描述为 **启发式的迭代 BFS**, 精度为 `double`.

## CLI 使用

### 构建

```bash
cargo build --release
```

### 运行示例

```bash
cargo run --release -- -T 114514 -m 4,5,5 -n 3 -k 64 -r 16 --constants e,pi,phi
```

### 参数说明

- `-T, --target <FLOAT>`
  - 目标值
- `-m, --max-ops <INT[,INT,...]>`
  - 每轮块搜索深度. 可传逗号列表; 若轮数超过列表长度, 后续沿用最后一个值
  - 默认: `4`
- `-n, --terms <INT>`
  - 迭代轮数
  - 默认: `3`, 这意味着结果形如 $X \pm Y \pm Z$, 其中 $X, Y, Z$ 的深度都不超过 `max-ops`
- `-k, --top-k <INT>`
  - 每轮保留的候选池大小
  - 默认: `20`
- `-w, --workspace-size <INT>`
  - 单次块搜索可保留的总工作区预算
  - 默认: `5000000`
- `-g, --gen-limit <INT>`
  - 单次块搜索的生成上限
  - 默认: `100000000`
- `-c, --constants <NAME[,NAME,...]>`
  - 初始常量集合
  - 默认: `e,pi`
- `-r, --result-cnt <INT>`
  - 最终输出结果数量
  - 默认: `5`

## Sympy 验证

`analyze.py` 用于对表达式做符号化与 LaTeX/数值验证.

安装依赖:

```bash
pip install sympy
```

示例:

```bash
python3 analyze.py "((pi^2)+e)"
```

输出:

- 原式与化简式
- 展开式
- LaTeX 表达
- `64` 位精度的数值结果

## 算法概览

### Expression

- `ExprTree`: 表达式树, `Leaf` 为常量, `Node` 为二元算符节点
- `MathNode`: 包含表达式树, 数值 `val`, 复杂度 `complexity`, 去重和排序用的 `id`

### Block Search

给定目标 `target` 与深度上限 `max_ops`, 逐层生成表达式:

- 组合来源: 上一层与历史层做配对
- 可用算符: `+`, `-`, `*`, `/`, `^`
- 每层按与目标误差排序, 并按预算截断保留.

约束和启发:

- `workspace_size`: 按层线性分配保留容量, 深层权重更高
- `gen_limit`: 限制单层生成规模

实现细节:

- 第 $k$ 层的保留上限按权重 `1..=max_ops` 分配, 深层会拿到更高预算
- 展开时按 `max_gen_nodes` 做*均匀的*二次切片
- 候选生成时只在 `id(left) <= id(right)` 的条件下加入 `+` 和 `*`, 预剪枝

### Iterative Refinement

设当前表达式为 $E$, 目标为 $T$.

- 计算残差 $r_1 = T - E$, 搜索残差 $r_1$, 构造 $E + r_1$
- 计算残差 $r_2 = E - T$, 搜索残差 $r_2$, 构造 $E - r_2$

每轮对候选池中的每个表达式都做上面两次搜索, 汇总后:

1. 按误差优先, 复杂度次优排序
2. 去重并截断到 `top_k`
3. 进入下一轮

数值约束:

- 所有运算都要求结果 `is_finite()`
- 除法要求分母绝对值大于 `1e-24`
- 幂运算要求底数为正

会牺牲一部分搜索空间, 但能减少 `NaN/Inf` 污染.

## 复杂度分析

> [!IMPORTANT]
>
> 以下估计考虑参数截断情况下的最差可能运行时间.

记:

- $D$: 单次 block search 的 `max_ops`
- $K$: 候选池大小 `top_k`
- $N$: 迭代轮数 `terms`

### Block Search 复杂度

第 $\ell$ 层的组合数可写成:

$$
C_\ell = \sum_{i=0}^{\ell-1} |L_i|\cdot|L_{\ell-1-i}|
$$

其中 $L_i$ 是第 $i$ 层保留节点集合. 每个 pair 最多尝试常数个算符, 所以生成代价是 $O(C_\ell)$.

该实现里, `gen_limit` 限制每层展开量, 因此实际 $C_\ell$ 通常被截断.

每层还需要排序/截断, 设本层候选数为 $M_\ell$:

- 若 $M_\ell > {\text{limit}}_\ell$:

  `select_nth` + 局部排序, 约 $O(M_\ell + \text{limit} _\ell\log\text{limit} _\ell)$

- 否则: 全排序 $O(M_\ell\log M_\ell)$

所以单次 block search 总时间是:

$$
T_{block}(D) = \sum_{\ell=1}^{D} \Big(O(C_\ell) + O(\text{sort}_\ell)\Big)
$$

空间复杂度主要来自各层 pool:

$$
S_{block}(D) = O\left(\sum_{\ell=0}^{D}|L_\ell|\right)
$$

### Iterative Refinement 总复杂度

第 $s$ 轮 (`s>=2`) 会对池中每个表达式做两次 block search, 搜索计算量约:

$$
O\big(2K\cdot T_{block}(m_s)\big)
$$

其中 $m_s$ 是该轮使用的 `max_ops`.

此外, 每个表达式最多产出约 $2K$ 个候选, 整轮合并规模上界约 $2K^2$, 排序计算量约:

$$
O(K^2\log K)
$$

因此总时间为:

$$
T_{total} \approx T_{block}(m_1) + \sum_{s=2}^{N}\left(2K\cdot T_{block}(m_s) + O(K^2\log K)\right)
$$

### 渐进时间复杂度估计

> [!IMPORTANT]
>
> 以下估计:
>
> 1. **不**考虑 `workspace_size` 和 `gen_limit` 的截断
> 2. **不**考虑数值剪枝或后剪枝
> 3. **考虑**基于 `id` 的预剪枝

设初始常量数量为 $c$, 令 $a_k$ 为恰好含 $k$ 次二元运算的可用表达式数量.

叶节点:

$$
a_0 = c
$$

对 $k \geq 1$, 令

$$
S_{k-1} = \sum_{i=0}^{k-1} a_i \cdot a_{k-1-i}
$$

为深度为 $i$ 和深度为 $k-1-i$ 的子树对的卷积.

#### 非对称算符 (-, /, ^)

非对称算符输入的有序对 $(e_l, e_r)$ 各自构成不同的表达式, 共 $3S_{k-1}$ 种.

#### 对称算符 (+, *)

为了消除对称算符造成的重复, 算法规定仅在 $\mathrm{id}(e_l) \le \mathrm{id}(e_r)$ 时生成表达式, 等价于从可用子树集合中提取**可重无序对**.

设 $m = (k-1)/2$, 无序对总数 $U_{k-1}$ 为:

$$
U_{k-1} = \frac{S_{k-1}}{2} + \frac{[2\mid(k-1)] \cdot a_m}{2}
$$

其中:

- **跨深度无序对**：当左右子树运算量 $i \neq j$ 时, 有序对数量为 $S_{k-1} - [2\mid(k-1)]a_m^2$. 由于 $i, j$ 对称, 取一半
- **同深度无序对**：当 $k-1$ 为偶数时, 可能出现左右子树运算量相同 ($i=j=m$) 的情况, 此时需从 $a_m$ 个表达式中挑选 2 个, 组合数为 $\binom{a_m+1}{2} = \frac{a_m(a_m+1)}{2}$

所以, 对称算符构成的表达式数量为:

$$
2 \cdot U_{k-1} = S_{k-1} + [2\mid(k-1)] \cdot a_m
$$

#### 总表达式数量

两者相加:

$$
a_k = 4S_{k-1} + [2\mid(k-1)]\cdot a_{(k-1)/2}
$$

总表达式数量:

$$
A(d) = \sum_{k=0}^{d} a_k
$$

#### 数值展开 ($c=2$)

| $k$ | $a_k$ | $A(k)$ |
| :---: | ---: | ---: |
| 0 | $2$ | $2$ |
| 1 | $4\cdot4+2=18$ | $20$ |
| 2 | $4\cdot72=288$ | $308$ |
| 3 | $4\cdot1476+18=5{,}922$ | $6{,}230$ |
| 4 | $4\cdot34056=136{,}224$ | $142{,}454$ |
| 5 | $4\cdot841032+288=3{,}364{,}416$ | $3{,}506{,}870$ |

$a_k$ 增长足够快, $A(d)\approx a_d$.

#### 渐进增长行为

将 $a_k$ 递推视为生成函数 $f(x)=\sum_{k\ge0}a_k x^k$, 忽略低阶修正项, 注意到:

$$
f(x) - c \approx 4xf(x)^2
\implies f(x) = \frac{1-\sqrt{1-16cx}}{8x}
= c\sum_{k=0}^{\infty}C_k(4c)^kx^k
$$

其中 $C_k=\frac{1}{k+1}\binom{2k}{k}$ 为第 $k$ 个卡特兰数，利用 $C_k\sim\frac{4^k}{k^{3/2}\sqrt\pi}$, 可得:

$$
a_k \sim \frac{c}{\sqrt\pi}\cdot\frac{(16c)^k}{k^{3/2}}
$$

即每增加一层运算, 表达式数量约乘以 $16c$. 对默认 `e,pi` ($c=2$) 约乘以 **32**, 对 `e,pi,phi` ($c=3$) 约乘以 **48**, 以此类推.

#### 代入复杂度

单次 block search 需要遍历同量级的候选:

$$
T_{block}(d) = \Theta\left(\sum_{k=0}^{d} a_k\right) \approx \Theta(a_d) = \Theta\left(\frac{(16c)^d}{d^{3/2}}\right)
$$

总时间估计为:

$$
T_{total}=\Theta\left(\frac{(16c)^{m_1}}{m_1^{3/2}}+2K\sum_{s=2}^{N}\frac{(16c)^{m_s}}{m_s^{3/2}}\right)
$$

若各轮深度相同 $m$:

$$
T_{total}=\Theta\left((1+2K(N-1))\cdot\frac{(16c)^m}{m^{3/2}}\right)
$$

结论是，`max_ops` 每增加 1, **总**运行时间约乘以 $16c$; 就时间复杂度来说, `max_ops` 是最敏感参数.

## 可用常量

| 符号 | 名称 (Name) | 约等于 |
| :--- | :--- | :--- |
| **1** | 单位元 (Unity, $1$) | $1.00000$ |
| **e** | 自然常数 (Euler's Number, $e$) | $2.71828$ |
| **pi** | 圆周率 (Archimedes' Constant, $\pi$) | $3.14159$ |
| **phi** | 黄金分割比 (Golden Ratio, $\phi$) | $1.61803$ |
| **sqrt2** | 2的算术平方根 (Pythagoras' Constant, $\sqrt{2}$) | $1.41421$ |
| **ln2** | 2的自然对数 (Natural Log of 2, $\ln{2}$) | $0.69315$ |
| **gamma** | 欧拉-马斯克若尼常数 (Euler-Mascheroni Constant, $\gamma$) | $0.57722$ |
| **C** | 卡塔兰常数 (Catalan's Constant, $C$) | $0.91597$ |
| **zeta3** | 阿培里常数 (Apéry's Constant, $\zeta{(3)}$) | $1.20206$ |
| **A** | 格莱舍-金克林常数 (Glaisher-Kinkelin Constant, $A$) | $1.28243$ |
| **delta** | 第一费根鲍姆常数 (First Feigenbaum Constant, $\delta$) | $4.66920$ |
| **alpha** | 第二费根鲍姆常数 (Second Feigenbaum Constant, $\alpha$) | $2.50291$ |
