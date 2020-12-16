const path = require('path')
const CopyWebpackPlugin = require('copy-webpack-plugin')

module.exports = {
  mode: process.env.WEBPACK_BUILD === 'production' ? 'production' : 'development',
  entry: './www/entry.js',
  output: {
    path: path.join(__dirname, 'dist'),
    filename: 'entry.js'
  },

  resolve: {
    extensions: ['.ts', '.js'],
  },

  experiments: {
    syncWebAssembly: true,
  },

  module: {
    rules: [
      {
        test: /\.ts$/,
        use: [
          {
            loader: 'ts-loader',
          }
        ]
      }
    ]
  },
  plugins: [
    new CopyWebpackPlugin({
        patterns: [
            'www/index.html'
        ]
    })
  ],
};