import sympy


def analyze_expression(expr_str: str):
    clean_expr = expr_str.replace('^', '**').lower()
    
    sym_constants = {
        '1': sympy.Integer(1),
        'e': sympy.E,
        'pi': sympy.pi,
        'phi': sympy.GoldenRatio,
        'gamma': sympy.EulerGamma,
        
        'sqrt2': sympy.Symbol('sqrt2', real=True),
        'ln2': sympy.Symbol('ln2', real=True),
        'c': sympy.Symbol('G', real=True),
        'zeta3': sympy.Symbol('zeta3', real=True),
        'a': sympy.Symbol('A', real=True),
        'delta': sympy.Symbol('delta', real=True),
        'alpha': sympy.Symbol('alpha', real=True),
    }

    latex_naming = {
        sym_constants['sqrt2']: r"\sqrt{2}",
        sym_constants['ln2']: r"\ln(2)",
        sym_constants['zeta3']: r"\zeta(3)",
    }
    
    expr = sympy.parse_expr(clean_expr, local_dict=sym_constants)
    
    simplified_expr = sympy.simplify(expr)
    expanded_expr = sympy.expand(simplified_expr)
    
    latex_opt = {"ln_notation": True, "symbol_names": latex_naming}
    latex_original = sympy.latex(expr, **latex_opt)
    latex_simplified = sympy.latex(simplified_expr, **latex_opt)
    latex_expanded = sympy.latex(expanded_expr, **latex_opt)
    
    val_map = {
        sym_constants['sqrt2']: sympy.sqrt(2),
        sym_constants['ln2']: sympy.ln(2),
        sym_constants['zeta3']: sympy.zeta(3),
        sym_constants['c']: sympy.Catalan,
        sym_constants['a']: sympy.Float("1.282427129100622636875342568869791727767688927325001192063740021740"),
        sym_constants['delta']: sympy.Float("4.66920160910299067185320382046620161725818557747576863274565134300"),
        sym_constants['alpha']: sympy.Float("2.50290787509589282228390287321821578638127137672714997733619205678")
    }
    
    precision = 64
    numeric_value = simplified_expr.subs(val_map).evalf(precision)
    
    return {
        "original": expr,
        "latex_original": latex_original,
        "simplified": simplified_expr,
        "expanded": expanded_expr,
        "latex_simplified": latex_simplified,
        "latex_expanded": latex_expanded,
        "value": numeric_value
    }

if __name__ == "__main__":
    import sys
    
    raw_expr = r"(delta + sqrt2 * A)^2"
    
    if len(sys.argv) > 1:
        raw_expr = sys.argv[1]
    
    print(f"Analyzing: {raw_expr}")
    result = analyze_expression(raw_expr)
    
    print("\n[LaTeX Original]")
    print(result["latex_original"])
    
    print("\n[LaTeX Simplified]")
    print(result["latex_simplified"])
    
    print("\n[LaTeX Expanded]")

    print(result["latex_expanded"])
    
    print("\n[Numeric]")
    print(result["value"])