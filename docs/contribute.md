# 贡献说明

Muta 是完全开源的公链项目，代码在 GitHub 托管，欢迎社区成员以各种方式参与贡献。

## 提 Issue
 
 欢迎任何帮助性的建议[issue](https://github.com/nervosnetwork/muta/issues)。如果是 bug，请附上详细的复现步骤或者对应的日志。

 ## 提 PR

 1. Fork muta 到自己的仓库中并 clone 到本地

    ```
    $ git clone https://github.com/<your github id>/muta.git
    ```

 2. 创建新的分支
   
    ```
    $ git checkout -b <new branch name>
    ```

    分支名应尽量简洁并能体现出该分支完成的工作。

3. 先新建的分支上增加一些新特性或解决一些 bug
   
   * 一个 Pull Request 应该只关注于一件事情，如添加功能，修补 bug，重构代码，翻译文档。
   * 新增的代码编码风格参照项目主分支风格，尽量保持于主分支编码风格相同。

4. 提交修改
   
   在提交之前，首先通过 git add 命令添加修改文件到暂存区, 然后：
   
   ```
   $ git commit -m "commit message"
   ```
   
5. 将修改上传到你的仓库
   
   ```
   git push origin <branch name>
   ```

6. 创建并提交 PR

   在同步完成后，即可通过 Create pull request 按钮将该分支发送给该项目的维护者等待被合并。如果你的 Pull Request 修复了在 Issues 中描述的问题，可以使用 [special magic keyword](https://help.github.com/en/github/managing-your-work-on-github/linking-a-pull-request-to-an-issue) 引用该 Issue 为参考。

以上步骤，如有对 git 命令不熟悉的，请参考[git](https://git-scm.com/doc) 使用手册。

